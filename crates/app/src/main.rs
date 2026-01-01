use controller::UserController;
use model::SqliteUserModel;
use view::render_user_profile;
use app::{ThreadPool, get_home, get_not_found};
use config::get_config;
use std::{
    thread,
    process::{Child, ChildStderr, ChildStdout, Command, Stdio},
    time::{Instant, Duration},
    backtrace::Backtrace,
    collections::VecDeque,
    io::{self, BufReader, prelude::*, ErrorKind},
    net::{TcpListener, TcpStream},
    os::unix::io::{AsRawFd, RawFd},
};
use mio::{Events, Interest, Poll, Token};
use mio::unix::SourceFd;
use rand; 
use tungstenite::{accept, Message, Bytes, WebSocket};


// importing like this is nice because all files end up in the binary and stay in RAM for quick
// access. Also means you just ship the binary instead of files.
static CUSTOM_HTMX_JS: &[u8] = include_bytes!("../../../static/custom_htmx.js"); 
static FIRE_TRUCK_SVG: &[u8] = include_bytes!("../../../static/firetruck.svg"); 
static AMBULANCE_TRUCK_SVG: &[u8] = include_bytes!("../../../static/ambulance.svg"); 
static POLICE_SVG: &[u8] = include_bytes!("../../../static/police.svg"); 
static ANIMATION_CSS: &[u8] = include_bytes!("../../../static/animation.css"); 
// static WASM_HELLO: &[u8] = include_bytes!("../wasm-hello/pkg/wasm_hello.js");
// static WASM_HELLO_RUST: &[u8] = include_bytes!("../wasm-hello/pkg/wasm_hello_bg.wasm");

fn main() {
    let _ = db::pool();
    let model = SqliteUserModel::new();
    let controller = UserController::new(model);

    let seeded = controller.create_user("first-user", "first@example.com")
        .unwrap_or_else(|e| panic!("error, when creating user. Error: {e}"));
    match controller.get_user(seeded.id())
        .unwrap_or_else(|e| panic!("error, when fetching user. Error: {e}")) {
        Some(user) => println!("{}", render_user_profile(&user)),
        None => eprintln!("User not found"),
    };

    // websocket threads
    thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:8787").unwrap();
        let max_users = get_config().max_users;
        let pool = ThreadPool::new(max_users); 
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    pool.execute(|| {
                        handle_websocket_connection(s);
                    });
                }
                Err(e) => eprintln!("websocket connection from browser failed. {e}"),
            }
        }
    });

    // http threads
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let pool = ThreadPool::new(4); // for serving http files needed to get the websocket setup
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                pool.execute(|| {
                    handle_http_connection(s);
                });
            }
            Err(e) => eprintln!("connection from browser client failed. {e}"),
        }
    }
}


fn handle_http_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&stream);
    let request_line = buf_reader.lines().next();
    let request_line = match request_line {
        Some(request_line) => match request_line {
            Ok(request_line) => request_line,
            Err(err) => {
                println!("error, when reading request line {err}");
                return;
            }
        },
        None => {
            println!("error, no request line found");
            return;
        }
    };
    let (status_line, contents, content_type, enable_cache): (&str, &[u8], &str, bool) = match &request_line[..] {
        "GET / HTTP/1.1" => (
            "HTTP/1.1 200 OK",
            &get_home(&get_config().services),
            "text/html; charset=utf-8",
            false,
        ),
        "GET /static/custom_htmx.js HTTP/1.1" => (
            "HTTP/1.1 200 OK",
            CUSTOM_HTMX_JS,
            "application/javascript; charset=utf-8",
            true,
        ),
        "GET /static/animation.css HTTP/1.1" => (
            "HTTP/1.1 200 OK",
            ANIMATION_CSS,
            "text/css",
            true,
        ),
        "GET /static/firetruck.svg HTTP/1.1" => (
            "HTTP/1.1 200 OK",
            FIRE_TRUCK_SVG,
            "image/svg+xml",
            true,
        ),
        "GET /static/ambulance.svg HTTP/1.1" => (
            "HTTP/1.1 200 OK",
            AMBULANCE_TRUCK_SVG,
            "image/svg+xml",
            true,
        ),
        "GET /static/police.svg HTTP/1.1" => (
            "HTTP/1.1 200 OK",
            POLICE_SVG,
            "image/svg+xml",
            true,
        ),
        // "GET /static/wasm_hello.js HTTP/1.1" => (
        //     "HTTP/1.1 200 OK",
        //     WASM_HELLO,
        //     "application/javascript; charset=utf-8",
        //     true,
        // ),
        // "GET /static/wasm_hello_bg.wasm HTTP/1.1" => (
        //     "HTTP/1.1 200 OK",
        //     WASM_HELLO_RUST,
        //     "application/wasm",
        //     true,
        // ),
        _ => (
            "HTTP/1.1 404 NOT FOUND",
            &get_not_found(),
            "text/html; charset=utf-8",
            true,
        ),
    };


    let headers = format!(
        "{status_line}\r\nContent-Type: {content_type}\r\n{}Content-Length: {}\r\n\r\n",
        if enable_cache { "Cache-Control: public, max-age=86400\r\n" } else { "" },
        contents.len(),
    );
    println!("serving request: {status_line}");
    let result = stream.write_all(&headers.into_bytes());
    match result {
        Ok(()) => (),
        Err(err) => {
            let bt = Backtrace::capture();
            eprintln!("error, when streaming headers to client. Error: {}. Stack: {:?}", err, bt);
            return
        }
    }

    let result = stream.write_all(contents);
    match result {
        Ok(()) => (),
        Err(err) => {
            let bt = Backtrace::capture();
            eprintln!("error, when streaming content to client. Error: {}. Stack: {:?}", err, bt);
            return
        }
    }
}

const SOCKET: Token = Token(0);
const STDOUT: Token = Token(2);
const STDERR: Token = Token(3);

fn handle_websocket_connection(stream: TcpStream) {
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

    let mut outbox: VecDeque<Message> = VecDeque::new();
    let mut deploy: Option<DeployChild> = None;
    let mut want_write = false;

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
) {
    // custom ping / pong started by client since the client doesn't know when it can reconnect due to no
    // access to control frames
    let message = msg.to_string();
    if message == "ping" {
        outbox.push_back(Message::Text("pong".into()));
    } else {
        let response = format!("deploying {}", message);
        outbox.push_back(Message::Text(response.into()));
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
    let write_work_needed = !outbox.is_empty();
    if *want_write != write_work_needed {
        *want_write = write_work_needed;
        let _ = update_socket_interest(poll, socket_source, *want_write);
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
