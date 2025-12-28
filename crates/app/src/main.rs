use controller::UserController;
use model::SqliteUserModel;
use view::render_user_profile;
use app::{ThreadPool, get_home, get_not_found};
use config::get_config;
use std::{
    thread,
    process::{Command, Stdio},
    time::{Instant, Duration},
    backtrace::Backtrace,
    io::{self, BufReader, prelude::*, ErrorKind},
    net::{TcpListener, TcpStream},
    sync::{OnceLock, mpsc},
};
use rand; 
use tungstenite::{accept, Message, Bytes, WebSocket};


// importing like this is nice because all files end up in the binary and stay in RAM for quick
// access. Also means you just ship the binary instead of files.
static CUSTOM_HTMX_JS: &[u8] = include_bytes!("../static/custom_htmx.js"); 
static WASM_HELLO: &[u8] = include_bytes!("../wasm-hello/pkg/wasm_hello.js");
static WASM_HELLO_RUST: &[u8] = include_bytes!("../wasm-hello/pkg/wasm_hello_bg.wasm");

static WATCHER_POOL: OnceLock<ThreadPool> = OnceLock::new();
fn get_watcher_pool() -> ThreadPool {
    WATCHER_POOL.get_or_init(|| ThreadPool::new(get_config().max_users * 2)) // each user will need two threads one
}

static DEPLOYMENT_POOL: OnceLock<ThreadPool> = OnceLock::new();
fn get_deployment_pool() -> ThreadPool {
    DEPLOYMENT_POOL.get_or_init(|| ThreadPool::new(get_config().max_users)) 
}


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

    let _ = get_watcher_pool();
    // watcher threads for watching child process stderr and stdout

    // websocket threads
    thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:8787").unwrap();
        let pool = ThreadPool::new(CONCURRENT_USERS_SUPPORTED); 
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
            &get_home(),
            "text/html; charset=utf-8",
            false,
        ),
        "GET /static/custom_htmx.js HTTP/1.1" => (
            "HTTP/1.1 200 OK",
            CUSTOM_HTMX_JS,
            "application/javascript; charset=utf-8",
            true,
        ),
        "GET /static/wasm_hello.js HTTP/1.1" => (
            "HTTP/1.1 200 OK",
            WASM_HELLO,
            "application/javascript; charset=utf-8",
            true,
        ),
        "GET /static/wasm_hello_bg.wasm HTTP/1.1" => (
            "HTTP/1.1 200 OK",
            WASM_HELLO_RUST,
            "application/wasm",
            true,
        ),
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

    let result = websocket.get_mut().set_read_timeout(Some(Duration::from_millis(250)));
    match result {
        Ok(()) => (),
        Err(err) => {
            let bt = Backtrace::capture();
            eprintln!("error, unable to set read timeout for handling websocket connection. Error: {}. Stack: {:?}", err, bt);
            return
        }
    }

    loop {
        match websocket.read() {
            Ok(msg) => {
                // Any inbound traffic counts as “alive”
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
                            .unwrap_or_else(|e| eprintln!("error, when sending close response in response to close request. Error: {}", err));
                        break;
                    }
                    other => {
                        handle_app_message(&mut websocket, other);
                    }
                }
            }
            Err(e) if is_timeout(&e) => {
                // No message arrived within read_timeout; fall through to timer checks.
            }
            Err(err) => {
                let bt = Backtrace::capture();
                eprintln!("error, when reading websocket message. Error: {}. Stack: {:?}", err, bt);
                return
            }
        }

        let now = Instant::now();

        // 1) If we've been idle long enough, ping.
        if now.duration_since(last_rx) >= ping_interval && ping_in_flight.is_none() {
            let result = websocket.send(Message::Ping(Bytes::new()));
            match result {
                Ok(()) => (),
                Err(err) => {
                    eprintln!("error, when writing ping websocket message. Error: {}", err);
                    return
                }
            }
            ping_in_flight = Some(now);
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



fn handle_app_message(websocket: &mut WebSocket<TcpStream>, msg: Message) {
    // custom ping / pong started by client since the client doesn't know when it can reconnect due to no
    // access to control frames
    let message = msg.to_string();
    if message == "ping" {
        let result = websocket.send(Message::Text("pong".into()));
        match result {
            Ok(()) => (),
            Err(err) => {
                let bt = Backtrace::capture();
                eprintln!("error, when writing to websocket connection for pong. Error: {}. Stack: {:?}", err, bt);
                return
            }
        }
    } else {
        let response = format!("deploying {}", message);
        let result = websocket.send(Message::Text(response.into()));
        match result {
            Ok(()) => (),
            Err(err) => {
                let bt = Backtrace::capture();
                eprintln!("error, when writing to websocket connection. Error: {}. Stack: {:?}", err, bt);
                return
            }
        }
        let rx = start_deploy();
        let mut got = 0;
        loop {
            match rx.recv() {
                Ok(DeployEvent::Output(message)) => {
                    if let Err(e) = websocket.send(Message::Text(message.into())) {
                        eprintln!("error, when writing to websocket connection for streaming stdout. Error: {}", e);
                        break;
                    };
                }
                Ok(DeployEvent::Done) => {
                    got += 1;
                    if got == 2 {
                        break;
                    }
                }
                Ok(DeployEvent::Error(message)) => {
                    let _ = websocket.send(Message::Text(message.into()));
                    break;
                }
                Err(_) => break, // all senders dropped
            }
        }
    }
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

enum DeployEvent {
    Output(String),
    Done,
    Error(String),
}

fn start_deploy() -> mpsc::Receiver<DeployEvent> {
    let (tx, rx) = mpsc::channel::<DeployEvent>();
    let tx_error = tx.clone();
    get_deployment_pool().execute(move || {
        if let Err(err) = run_deploy(tx) {
            let msg = format!("deploy failed: {}", err);
            eprintln!("error, when running deploy: {}", msg);
            let _ = tx_error.send(DeployEvent::Error(msg));
        }
    });
    rx
}

fn run_deploy(tx: mpsc::Sender<DeployEvent>) -> Result<(), std::io::Error> {
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

    {
        let tx = tx.clone();
        WATCHER_POOL.get().expect("watcher pool did not init").execute(move || {
            let reader = BufReader::new(stdout);
            for line_res in reader.lines() {
                let line = match line_res {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("error, when reading watcher pool lines for stdout: {}", e);
                        break;
                    }
                };
                // safe to ignore here because error would just mean the receiver is already dropped
                let _ = tx.send(DeployEvent::Output(line.to_string()));
            }
            // safe to ignore here because error would just mean the receiver is already dropped
            let _ = tx.send(DeployEvent::Done);
        });
    }
    {
        let tx = tx.clone();
        WATCHER_POOL.get().expect("watcher pool did not init").execute(move || {
            let reader = BufReader::new(stderr);
            for line_res in reader.lines() {
                let line = match line_res {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("error, when reading watcher pool lines for stderr: {}", e);
                        break;
                    }
                };
                // safe to ignore here because error would just mean the receiver is already dropped
                let _ = tx.send(DeployEvent::Output(line.to_string()));
            }
            // safe to ignore here because error would just mean the receiver is already dropped
            let _ = tx.send(DeployEvent::Done);
        });
    }
    let status = child.wait().map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("error, when waiting for child process. Error: {}", e),
        )
    })?;
    println!("child process exited: {status}");
    Ok(())
}
