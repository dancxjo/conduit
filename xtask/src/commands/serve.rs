use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::{Path, PathBuf},
};

pub fn run(directory: PathBuf, requested_port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("127.0.0.1:{requested_port}"))?;
    let local_addr = listener.local_addr()?;
    let port = local_addr.port();

    println!("READY:{port}");
    let _ = std::io::stdout().flush();

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };

        let root_dir = directory.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(&stream);
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
                return;
            }

            let parts: Vec<&str> = request_line.split_whitespace().collect();
            if parts.len() < 2 || parts[0] != "GET" {
                let _ = stream.write_all(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n");
                return;
            }

            let raw_path = parts[1];
            let url_path = raw_path.split('?').next().unwrap_or(raw_path);
            let rel_path = url_path.trim_start_matches('/');
            let target_path = root_dir.join(rel_path);

            let canonical_target = match target_path.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n");
                    return;
                }
            };
            let canonical_root = match root_dir.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    let _ = stream.write_all(b"HTTP/1.1 500 Internal Error\r\n\r\n");
                    return;
                }
            };

            if !canonical_target.starts_with(&canonical_root) {
                let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n");
                return;
            }

            if canonical_target.is_dir() {
                let index = canonical_target.join("index.html");
                if index.is_file() {
                    send_file(&mut stream, &index);
                } else {
                    let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n");
                }
            } else if canonical_target.is_file() {
                send_file(&mut stream, &canonical_target);
            } else {
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n");
            }
        });
    }

    Ok(())
}

fn send_file(stream: &mut std::net::TcpStream, path: &Path) {
    let content = match fs::read(path) {
        Ok(c) => c,
        Err(_) => {
            let _ = stream.write_all(b"HTTP/1.1 500 Internal Error\r\n\r\n");
            return;
        }
    };

    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let mime = match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "wasm" => "application/wasm",
        "panel" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    };

    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        mime,
        content.len()
    );

    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(&content);
}
