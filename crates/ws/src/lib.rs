use controller::{AppEvent, ParseEventError, UiMode, UiResult, handle_nav, parse_event, parse_query_params};
use config::get_config;
use std::{
    backtrace::Backtrace,
    collections::VecDeque,
    io::{self, ErrorKind, Read},
    net::TcpStream,
    os::unix::io::{AsRawFd, RawFd},
    process::{Child, ChildStderr, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};
use mio::{Events, Interest, Poll, Token};
use mio::unix::SourceFd;
use tungstenite::{accept, Bytes, Message, WebSocket};

const SOCKET: Token = Token(0);
const STDOUT: Token = Token(2);
const STDERR: Token = Token(3);

pub fn handle_websocket_connection(stream: TcpStream) {
    let config = get_config();
    let websocket = accept(stream);
    let mut websocket = match websocket {
        Ok(w) => w,
        Err(err) => {
            let bt = Backtrace::capture();
            eprintln!("error, when accepting websocket connection. Error: {}. Stack: {:?}", err, bt);
            return
        }
    };

    let ping_interval = Duration::from_secs(rand::random_range(20..=30));
    let pong_timeout = Duration::from_secs(rand::random_range(7..=10));

    let mut last_rx = Instant::now();
    let mut ping_in_flight: Option<Instant> = None;

    if let Err(err) = websocket.get_mut().set_nonblocking(true) {
        let bt = Backtrace::capture();
        eprintln!("error, unable to set non-blocking for websocket. Error: {}. Stack: {:?}", err, bt);
        return
    }

    let mut poll = match Poll::new() {
        Ok(p) => p,
        Err(err) => {
            let bt = Backtrace::capture();
            eprintln!("error, when creating websocket poll. Error: {}. Stack: {:?}", err, bt);
            return
        }
    };
    let mut events = Events::with_capacity(64);

    let raw_fd = websocket.get_ref().as_raw_fd();
    let mut socket_source = SourceFd(&raw_fd);
    if let Err(err) = poll.registry().register(
            &mut socket_source,
            SOCKET,
            Interest::READABLE,
        ) {
        let bt = Backtrace::capture();
        eprintln!("error, when registering websocket socket with poll. Error: {}. Stack: {:?}", err, bt);
        return
    }

    let ready_message = format!("ready:{}", config.app_version);
    let mut outbox: VecDeque<Message> = VecDeque::from([Message::Text(ready_message.into())]);
    let mut deploy: Option<DeployChild> = None;
    let mut want_write = false;
    if drain_outbound(&mut outbox, &mut websocket, &mut ping_in_flight).is_err() {
        return
    }
    let write_work_needed = !outbox.is_empty();
    if want_write != write_work_needed {
        want_write = write_work_needed;
        if update_socket_interest(&mut poll, &mut socket_source, want_write).is_err() {
            return
        }
    }

    loop {
        let now = Instant::now();
        let ping_deadline = last_rx + ping_interval;
        let timeout = match ping_in_flight {
            Some(t0) => {
                let pong_deadline = t0 + pong_timeout;
                let deadline = if ping_deadline < pong_deadline { ping_deadline } else { pong_deadline };
                deadline.saturating_duration_since(now)
            }
            None => ping_deadline.saturating_duration_since(now),
        };
        if let Err(err) = poll.poll(&mut events, Some(timeout)) {
            let bt = Backtrace::capture();
            eprintln!("error, when polling websocket. Error: {}. Stack: {:?}", err, bt);
            return
        }

        for event in events.iter() {
            match event.token() {
                SOCKET => {
                    if event.is_readable() {
                        loop {
                            match websocket.read() {
                                Ok(msg) => {
                                    // Any inbound traffic counts as "alive"
                                    last_rx = Instant::now();
                                    ping_in_flight = None;
                                    match msg {
                                        Message::Ping(_) => {
                                            // tungstenite will auto-reply to ping/pongs but we still list them in this
                                            // match statement so application logic handling doesn't get handed control
                                            // logic messages
                                        }
                                        Message::Pong(_) => {
                                            // good, client is alive, but we already cleared ping_in_flight because any
                                            // traffic counts as proof of life
                                        }
                                        Message::Close(frame) => {
                                            websocket.send(Message::Close(frame))
                                                .unwrap_or_else(|e| eprintln!("error, when sending close response in response to close request. Error: {}", e));
                                            return
                                        }
                                        other => {
                                                handle_app_message(
                                                    &mut poll,
                                                    &mut outbox,
                                                    &mut deploy,
                                                    &mut socket_source,
                                                    other,
                                                    &mut want_write,
                                                    config,
                                                );
                                        }
                                    }
                                }
                                Err(e) if is_timeout(&e) => break,
                                Err(err) => {
                                    let bt = Backtrace::capture();
                                    eprintln!("error, when reading websocket message. Error: {}. Stack: {:?}", err, bt);
                                    return
                                }
                            }
                        }
                    }
                    if event.is_writable() {
                        if drain_outbound(&mut outbox, &mut websocket, &mut ping_in_flight).is_err() {
                            return
                        }
                    }
                }
                STDOUT => {
                    if let Some(dep) = deploy.as_mut() {
                        if handle_child_readable(dep, &mut outbox, ChildStream::Stdout).is_err() {
                            return
                        }
                    }
                }
                STDERR => {
                    if let Some(dep) = deploy.as_mut() {
                        if handle_child_readable(dep, &mut outbox, ChildStream::Stderr).is_err() {
                            return
                        }
                    }
                }
                _ => {}
            }
        }

        if drain_outbound(&mut outbox, &mut websocket, &mut ping_in_flight).is_err() {
            return
        }

        let write_work_needed = !outbox.is_empty();
        if want_write != write_work_needed {
            want_write = write_work_needed;
            if update_socket_interest(&mut poll, &mut socket_source, want_write).is_err() {
                return
            }
        }

        if let Some(dep) = deploy.as_mut() {
            if dep.is_done() {
                if finalize_deploy(&mut poll, dep, &mut outbox).is_err() {
                    return
                }
                deploy = None;
                if drain_outbound(&mut outbox, &mut websocket, &mut ping_in_flight).is_err() {
                    return
                }
                let write_work_needed = !outbox.is_empty();
                if want_write != write_work_needed {
                    want_write = write_work_needed;
                    if update_socket_interest(&mut poll, &mut socket_source, want_write).is_err() {
                        return
                    }
                }
            }
        }

        let now = Instant::now();

        // 1) If we've been idle long enough, ping.
        if now.duration_since(last_rx) >= ping_interval && ping_in_flight.is_none() {
            match websocket.send(Message::Ping(Bytes::new())) {
                Ok(()) => ping_in_flight = Some(now),
                Err(err) if is_timeout(&err) => outbox.push_back(Message::Ping(Bytes::new())),
                Err(err) => {
                    eprintln!("error, when writing ping websocket message. Error: {}", err);
                    return
                }
            }
        }

        // 2) If we pinged and still didn't get anything back in time, close.
        if let Some(t0) = ping_in_flight {
            if now.duration_since(t0) >= pong_timeout {
                // you can also send Close first if you want
                let _ = websocket.send(Message::Close(None));
                break;
            }
        }
    }
}

fn handle_app_message(
    poll: &mut Poll,
    outbox: &mut VecDeque<Message>,
    deploy: &mut Option<DeployChild>,
    socket_source: &mut SourceFd,
    msg: Message,
    want_write: &mut bool,
    config: &'static config::AppConfig,
) {
    // custom ping / pong started by client since the client doesn't know when it can reconnect due to no
    // access to control frames
    match parse_event(&msg.to_string()) {
        Ok(AppEvent::Ping) => outbox.push_back(Message::Text("pong".into())),
        Ok(AppEvent::SearchServices(s)) => {
            println!("todo remove searching service {}", s);
        }
        Ok(AppEvent::Navigate(path)) => {
            let (path_only, query) = split_path_query(&path);
            let query_params = parse_query_params(query);
            match handle_nav(path_only, query_params, config, UiMode::Patch) {
                UiResult::Patch(html) => {
                    outbox.push_back(Message::Text(format!("patch:{}", html).into()));
                    outbox.push_back(Message::Text(format!("location:{}", path).into()));
                }
                UiResult::Redirect(location) => {
                    outbox.push_back(Message::Text(format!("location:{}", location).into()));
                }
                UiResult::FullHtml(_) | UiResult::NotFound(_) => {
                    outbox.push_back(Message::Text("error: invalid navigation result".into()));
                }
            }
        }
        Ok(AppEvent::Deploy(s)) => {
            outbox.push_back(Message::Text(format!("new_deployment: {}", s).into()));
            if deploy.is_some() {
                outbox.push_back(Message::Text("deploy already running".into()));
                return
            }
            match spawn_deploy() {
                Ok(mut dep) => {
                    if register_child_fds(poll, &mut dep).is_err() {
                        return
                    }
                    *deploy = Some(dep);
                }
                Err(err) => {
                    let msg = format!("deploy failed: {}", err);
                    eprintln!("error, when running deploy: {}", msg);
                    outbox.push_back(Message::Text(msg.into()));
                }
            }
        }
        Err(ParseEventError::UnknownKind) => outbox.push_back(Message::Text("error, unknown event kind".into())),
        Err(ParseEventError::MissingArg) => outbox.push_back(Message::Text("error, missing event arg".into())),
        Err(ParseEventError::ExtraData) => outbox.push_back(Message::Text("error, excess data in event call".into())),
    }
    let write_work_needed = !outbox.is_empty();
    if *want_write != write_work_needed {
        *want_write = write_work_needed;
        let _ = update_socket_interest(poll, socket_source, *want_write);
    }
}

fn split_path_query(path: &str) -> (&str, &str) {
    match path.split_once('?') {
        Some((path, query)) => (path, query),
        None => (path, ""),
    }
}

fn update_socket_interest(poll: &mut Poll, socket_source: &mut SourceFd, want_write: bool) -> Result<(), ()> {
    let interest = if want_write {
        Interest::READABLE.add(Interest::WRITABLE)
    } else {
        Interest::READABLE
    };
    if let Err(err) = poll.registry().reregister(socket_source, SOCKET, interest) {
        let bt = Backtrace::capture();
        eprintln!("error, when updating websocket socket interest. Error: {}. Stack: {:?}", err, bt);
        return Err(())
    }
    Ok(())
}

fn drain_outbound(
    outbox: &mut VecDeque<Message>,
    websocket: &mut WebSocket<TcpStream>,
    ping_in_flight: &mut Option<Instant>,
) -> Result<(), ()> {
    while let Some(msg) = outbox.pop_front() {
        let is_ping = matches!(msg, Message::Ping(_));
        match websocket.send(msg.clone()) {
            Ok(()) => {
                if is_ping && ping_in_flight.is_none() {
                    *ping_in_flight = Some(Instant::now());
                }
            }
            Err(err) if is_timeout(&err) => {
                outbox.push_front(msg);
                return Ok(())
            }
            Err(err) => {
                eprintln!("error, when writing to websocket connection. Error: {}", err);
                return Err(())
            }
        }
    }
    Ok(())
}

fn is_timeout(e: &tungstenite::Error) -> bool {
    use tungstenite::Error::Io;
    match e {
        Io(ioe) => {
            ioe.kind() == std::io::ErrorKind::WouldBlock
            || ioe.kind() == std::io::ErrorKind::TimedOut
        }
        _ => false,
    }
}

enum ChildStream {
    Stdout,
    Stderr,
}

struct DeployChild {
    child: Child,
    stdout: ChildStdout,
    stderr: ChildStderr,
    stdout_buf: Vec<u8>,
    stderr_buf: Vec<u8>,
    stdout_done: bool,
    stderr_done: bool,
    stdout_fd: RawFd,
    stderr_fd: RawFd,
}

impl DeployChild {
    fn is_done(&self) -> bool {
        self.stdout_done && self.stderr_done
    }
}

fn spawn_deploy() -> Result<DeployChild, std::io::Error> {
    let mut child = Command::new("ls")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("error, when writing to websocket connection for starting child process. Error: {}", e),
            )
        })?;

    let stdout = child.stdout.take().ok_or_else(|| {
        io::Error::new(
            ErrorKind::Other,
            "stdout wasn't piped or was already taken",
        )
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        io::Error::new(
            ErrorKind::Other,
            "stderr wasn't piped or was already taken",
        )
    })?;

    let stdout_fd = stdout.as_raw_fd();
    let stderr_fd = stderr.as_raw_fd();
    set_nonblocking_fd(stdout_fd)?;
    set_nonblocking_fd(stderr_fd)?;

    Ok(DeployChild {
        child,
        stdout,
        stderr,
        stdout_buf: Vec::new(),
        stderr_buf: Vec::new(),
        stdout_done: false,
        stderr_done: false,
        stdout_fd,
        stderr_fd,
    })
}

fn register_child_fds(poll: &mut Poll, deploy: &mut DeployChild) -> Result<(), ()> {
    let mut stdout_source = SourceFd(&deploy.stdout_fd);
    if let Err(err) = poll.registry().register(&mut stdout_source, STDOUT, Interest::READABLE) {
        let bt = Backtrace::capture();
        eprintln!("error, when registering deploy stdout. Error: {}. Stack: {:?}", err, bt);
        return Err(())
    }
    let mut stderr_source = SourceFd(&deploy.stderr_fd);
    if let Err(err) = poll.registry().register(&mut stderr_source, STDERR, Interest::READABLE) {
        let bt = Backtrace::capture();
        eprintln!("error, when registering deploy stderr. Error: {}. Stack: {:?}", err, bt);
        return Err(())
    }
    Ok(())
}

fn handle_child_readable(deploy: &mut DeployChild, outbox: &mut VecDeque<Message>, which: ChildStream) -> Result<(), ()> {
    match which {
        ChildStream::Stdout => {
            match read_child_stream(&mut deploy.stdout, &mut deploy.stdout_buf, outbox) {
                Ok(ChildRead::Progress) => Ok(()),
                Ok(ChildRead::Eof) => {
                    deploy.stdout_done = true;
                    Ok(())
                }
                Err(err) => {
                    eprintln!("error, when reading deploy output: {}", err);
                    Err(())
                }
            }
        }
        ChildStream::Stderr => {
            match read_child_stream(&mut deploy.stderr, &mut deploy.stderr_buf, outbox) {
                Ok(ChildRead::Progress) => Ok(()),
                Ok(ChildRead::Eof) => {
                    deploy.stderr_done = true;
                    Ok(())
                }
                Err(err) => {
                    eprintln!("error, when reading deploy output: {}", err);
                    Err(())
                }
            }
        }
    }
}

fn finalize_deploy(poll: &mut Poll, deploy: &mut DeployChild, outbox: &mut VecDeque<Message>) -> Result<(), ()> {
    let status = match deploy.child.try_wait() {
        Ok(Some(status)) => status,
        Ok(None) => return Ok(()),
        Err(err) => {
            eprintln!("error, when waiting for child process. Error: {}", err);
            return Err(())
        }
    };
    outbox.push_back(Message::Text(format!("child process exited: {status}").into()));
    let mut stdout_source = SourceFd(&deploy.stdout_fd);
    let _ = poll.registry().deregister(&mut stdout_source);
    let mut stderr_source = SourceFd(&deploy.stderr_fd);
    let _ = poll.registry().deregister(&mut stderr_source);
    Ok(())
}

enum ChildRead {
    Progress,
    Eof,
}

fn read_child_stream(stream: &mut impl Read, buf: &mut Vec<u8>, outbox: &mut VecDeque<Message>) -> io::Result<ChildRead> {
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => {
                flush_lines(buf, outbox);
                return Ok(ChildRead::Eof);
            }
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                flush_lines(buf, outbox);
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(ChildRead::Progress),
            Err(e) => return Err(e),
        }
    }
}

fn flush_lines(buf: &mut Vec<u8>, outbox: &mut VecDeque<Message>) {
    while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
        let mut line = buf.drain(..=pos).collect::<Vec<u8>>();
        if matches!(line.last(), Some(b'\n')) {
            line.pop();
        }
        if matches!(line.last(), Some(b'\r')) {
            line.pop();
        }
        let text = String::from_utf8_lossy(&line).to_string();
        outbox.push_back(Message::Text(text.into()));
    }
}

fn set_nonblocking_fd(fd: RawFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    let new_flags = flags | libc::O_NONBLOCK;
    let res = unsafe { libc::fcntl(fd, libc::F_SETFL, new_flags) };
    if res < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
