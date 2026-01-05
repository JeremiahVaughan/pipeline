use config::get_config;
use std::{
    backtrace::Backtrace,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
};
use view::{get_home, get_not_found};

// importing like this is nice because all files end up in the binary and stay in RAM for quick
// access. Also means you just ship the binary instead of files.
static CUSTOM_HTMX_JS: &[u8] = include_bytes!("../../../static/custom_htmx.js");
static FIRE_TRUCK_SVG: &[u8] = include_bytes!("../../../static/firetruck.svg");
static AMBULANCE_TRUCK_SVG: &[u8] = include_bytes!("../../../static/ambulance.svg");
static POLICE_SVG: &[u8] = include_bytes!("../../../static/police.svg");
static ANIMATION_CSS: &[u8] = include_bytes!("../../../static/animation.css");
// static WASM_HELLO: &[u8] = include_bytes!("../wasm-hello/pkg/wasm_hello.js");
// static WASM_HELLO_RUST: &[u8] = include_bytes!("../wasm-hello/pkg/wasm_hello_bg.wasm");

pub fn handle_http_connection(mut stream: TcpStream) {
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
