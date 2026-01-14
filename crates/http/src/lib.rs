use config::get_config;
use controller::{handle_nav, parse_query_params, UiMode, UiResult};
use std::{
    backtrace::Backtrace,
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
};
// importing like this is nice because all files end up in the binary and stay in RAM for quick
// access. Also means you just ship the binary instead of files.
static CUSTOM_HTMX_JS: &[u8] = include_bytes!("../../../static/custom_htmx.js");
static FIRE_TRUCK_SVG: &[u8] = include_bytes!("../../../static/firetruck.svg");
static AMBULANCE_TRUCK_SVG: &[u8] = include_bytes!("../../../static/ambulance.svg");
static POLICE_SVG: &[u8] = include_bytes!("../../../static/police.svg");
static ANIMATION_CSS: &[u8] = include_bytes!("../../../static/animation.css");
static LANDING_PAGE_CSS: &[u8] = include_bytes!("../../../static/landing_page.css");
static LANDING_PAGE_JS: &[u8] = include_bytes!("../../../static/landing_page.js");
static SETTINGS_PAGE_CSS: &[u8] = include_bytes!("../../../static/settings_page.css");
static SERVICE_PAGE_CSS: &[u8] = include_bytes!("../../../static/service_page.css");
static INTERNAL_ERROR_HTML: &[u8] = b"<html><body><h1>Internal Server Error</h1></body></html>";
// static WASM_HELLO: &[u8] = include_bytes!("../wasm-hello/pkg/wasm_hello.js");
// static WASM_HELLO_RUST: &[u8] = include_bytes!("../wasm-hello/pkg/wasm_hello_bg.wasm");

pub struct RequestLine {
    method: String,
    path: String,
    version: String,
}

impl RequestLine {
}

pub fn handle_http_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&stream);
    let request_line = buf_reader.lines().next();
    let (request_line, query_params) = match request_line {
        Some(request_line) => match request_line {
            Ok(request_line) => parse_request_line(request_line),
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
    let config = &get_config();
    let method = request_line.method.as_str();
    let path = request_line.path.as_str();
    let (status_line, contents, content_type, enable_cache): (&str, &[u8], &str, bool) = match (method, path) {
        ("GET", "/static/custom_htmx.js") => (
            "HTTP/1.1 200 OK",
            CUSTOM_HTMX_JS,
            "application/javascript; charset=utf-8",
            true,
        ),
        ("GET", "/static/landing_page.css") => (
            "HTTP/1.1 200 OK",
            LANDING_PAGE_CSS,
            "text/css",
            true,
        ),
        ("GET", "/static/landing_page.js") => (
            "HTTP/1.1 200 OK",
            LANDING_PAGE_JS,
            "application/javascript; charset=utf-8",
            true,
        ),
        ("GET", "/static/settings_page.css") => (
            "HTTP/1.1 200 OK",
            SETTINGS_PAGE_CSS,
            "text/css",
            true,
        ),
        ("GET", "/static/service_page.css") => (
            "HTTP/1.1 200 OK",
            SERVICE_PAGE_CSS,
            "text/css",
            true,
        ),
        ("GET", "/static/animation.css") => (
            "HTTP/1.1 200 OK",
            ANIMATION_CSS,
            "text/css",
            true,
        ),
        ("GET", "/static/firetruck.svg") => (
            "HTTP/1.1 200 OK",
            FIRE_TRUCK_SVG,
            "image/svg+xml",
            true,
        ),
        ("GET", "/static/ambulance.svg") => (
            "HTTP/1.1 200 OK",
            AMBULANCE_TRUCK_SVG,
            "image/svg+xml",
            true,
        ),
        ("GET", "/static/police.svg") => (
            "HTTP/1.1 200 OK",
            POLICE_SVG,
            "image/svg+xml",
            true,
        ),
        _ => {
            let ui_result = handle_nav(path, query_params, config, UiMode::FullPage);
            match ui_result {
                UiResult::FullHtml(html) => {
                    write_response(&mut stream, "HTTP/1.1 200 OK", "text/html; charset=utf-8", false, &html);
                    return;
                }
                UiResult::NotFound(html) => {
                    write_response(&mut stream, "HTTP/1.1 404 NOT FOUND", "text/html; charset=utf-8", true, &html);
                    return;
                }
                UiResult::Redirect(location) => {
                    let headers = format!(
                        "HTTP/1.1 302 FOUND\r\nLocation: {location}\r\nContent-Length: 0\r\n\r\n"
                    );
                    let _ = stream.write_all(&headers.into_bytes());
                    return;
                }
                UiResult::Patch(_) => {
                    write_response(&mut stream, "HTTP/1.1 500 INTERNAL SERVER ERROR", "text/html; charset=utf-8", false, INTERNAL_ERROR_HTML);
                    return;
                }
            }
        }
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

fn write_response(stream: &mut TcpStream, status_line: &str, content_type: &str, enable_cache: bool, contents: &[u8]) {
    let headers = format!(
        "{status_line}\r\nContent-Type: {content_type}\r\n{}Content-Length: {}\r\n\r\n",
        if enable_cache { "Cache-Control: public, max-age=86400\r\n" } else { "" },
        contents.len(),
    );
    if let Err(err) = stream.write_all(&headers.into_bytes()) {
        let bt = Backtrace::capture();
        eprintln!("error, when streaming headers to client. Error: {}. Stack: {:?}", err, bt);
        return;
    }
    if let Err(err) = stream.write_all(contents) {
        let bt = Backtrace::capture();
        eprintln!("error, when streaming content to client. Error: {}. Stack: {:?}", err, bt);
    }
}

fn parse_request_line(request_line: String) -> (RequestLine, HashMap<String, String>) {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default();
    let version = parts.next().unwrap_or_default().to_string();

    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, query),
        None => (target, ""),
    };

    let request_line = RequestLine {
        method,
        path: path.to_string(),
        version,
    };
    let query_params = parse_query_params(query);
    (request_line, query_params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_request_line_no_query_params() {
        let rl = "GET / HTTP/1.1".to_string();
        let (rl, qp) = parse_request_line(rl);
        assert_eq!(rl.method, "GET");
        assert_eq!(rl.path, "/");
        assert_eq!(rl.version, "HTTP/1.1");
        assert!(qp.is_empty());
    }


    #[test]
    fn parses_request_line_query_params() {
        let rl = "GET /settings?name=hello HTTP/1.1".to_string();
        let (rl, qp) = parse_request_line(rl);
        assert_eq!(rl.method, "GET");
        assert_eq!(rl.path, "/settings");
        assert_eq!(rl.version, "HTTP/1.1");
        let mut expected = HashMap::new();
        expected.insert("name".to_string(), "hello".to_string());
        assert_eq!(qp, expected);
    }
}
