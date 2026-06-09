use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::OnceLock;

static DOCS_PORT: OnceLock<u16> = OnceLock::new();

/// Returns the URL of the local docs server, starting it on first call.
pub fn docs_url() -> String {
    let port = *DOCS_PORT.get_or_init(start_server);
    format!("http://127.0.0.1:{port}/")
}

fn start_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind docs server");
    let port = listener.local_addr().unwrap().port();
    std::thread::Builder::new()
        .name("vassl-docs".into())
        .spawn(move || {
            for stream in listener.incoming() {
                if let Ok(stream) = stream {
                    serve_request(stream);
                }
            }
        })
        .expect("spawn docs server thread");
    port
}

fn serve_request(mut stream: std::net::TcpStream) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() { return; }

    // Consume remaining headers so the browser doesn't get a RST.
    for line in reader.lines() {
        match line {
            Ok(l) if l.is_empty() => break,
            Ok(_) => {}
            Err(_) => return,
        }
    }

    let url_path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();

    let asset_path = url_to_asset_path(&url_path);

    let (status, content_type, body): (&str, &str, std::borrow::Cow<[u8]>) =
        match crate::assets::VasslAssets::get(&asset_path) {
            Some(file) => ("200 OK", mime_for(&asset_path), file.data),
            None => (
                "404 Not Found",
                "text/plain",
                std::borrow::Cow::Borrowed(b"Not Found"),
            ),
        };

    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(&body);
}

/// Maps a URL path to an embedded asset path under the `docs/` prefix.
fn url_to_asset_path(url_path: &str) -> String {
    // Strip query string / fragment.
    let path = url_path.split(['?', '#']).next().unwrap_or("/");
    let trimmed = path.trim_start_matches('/');

    if trimmed.is_empty() {
        "docs/index.html".to_string()
    } else if trimmed.ends_with('/') {
        format!("docs/{trimmed}index.html")
    } else if !trimmed.contains('.') {
        // Directory-style URL without trailing slash — try index.html.
        format!("docs/{trimmed}/index.html")
    } else {
        format!("docs/{trimmed}")
    }
}

fn mime_for(path: &str) -> &'static str {
    if path.ends_with(".html")       { "text/html; charset=utf-8" }
    else if path.ends_with(".css")   { "text/css" }
    else if path.ends_with(".js")    { "application/javascript" }
    else if path.ends_with(".svg")   { "image/svg+xml" }
    else if path.ends_with(".png")   { "image/png" }
    else if path.ends_with(".webp")  { "image/webp" }
    else if path.ends_with(".ico")   { "image/x-icon" }
    else if path.ends_with(".woff2") { "font/woff2" }
    else if path.ends_with(".woff")  { "font/woff" }
    else if path.ends_with(".json")  { "application/json" }
    else                             { "application/octet-stream" }
}

/// Opens `url` in the system's default browser. Fire-and-forget.
pub fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/c", "start", url]).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
