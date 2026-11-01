//! Loopback-only transport adapter for browser-generated public signed proofs.
//! It holds no invitation/continuity key and cannot authorize membership.
use crate::Result;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::{Duration, Instant},
};
const HEADER_MAX: usize = 8192;
const BODY_MAX: usize = 1024;
fn route(method: &str, path: &str) -> bool {
    (method == "POST" || method == "OPTIONS") && path == "/body/join"
        || (method == "GET" || method == "OPTIONS")
            && path
                .strip_prefix("/body/join/result?id=")
                .is_some_and(|id| {
                    !id.is_empty()
                        && id.bytes().all(|b| b.is_ascii_digit())
                        && id.parse::<u32>().is_ok()
                })
}
fn read_until(stream: &mut TcpStream, mut out: &mut [u8], deadline: Instant) -> Result<()> {
    while !out.is_empty() {
        stream.set_read_timeout(Some(
            deadline
                .checked_duration_since(Instant::now())
                .ok_or("browser proxy deadline")?,
        ))?;
        let n = stream.read(out)?;
        if n == 0 {
            return Err("browser proxy closed".into());
        }
        out = &mut out[n..];
    }
    Ok(())
}
fn read_request(stream: &mut TcpStream) -> Result<(String, String, Vec<u8>)> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut header = Vec::new();
    let mut byte = [0];
    while !header.ends_with(b"\r\n\r\n") {
        if header.len() == HEADER_MAX {
            return Err("browser proxy header extent".into());
        }
        read_until(stream, &mut byte, deadline)?;
        header.push(byte[0]);
    }
    let header = std::str::from_utf8(&header)?;
    let mut lines = header.split("\r\n");
    let mut request = lines
        .next()
        .ok_or("browser proxy request")?
        .split_whitespace();
    let method = request.next().ok_or("browser proxy method")?;
    let path = request.next().ok_or("browser proxy path")?;
    if request.next() != Some("HTTP/1.1") || request.next().is_some() || !route(method, path) {
        return Err("browser proxy route".into());
    }
    let mut length = None;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or("browser proxy header")?;
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err("browser proxy transfer encoding".into());
        }
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err("browser proxy duplicate length".into());
            }
            let n = value.trim().parse::<usize>()?;
            if n > BODY_MAX {
                return Err("browser proxy body extent".into());
            }
            length = Some(n);
        }
    }
    let length = length.unwrap_or(0);
    if (method == "POST" && length == 0) || (method != "POST" && length != 0) {
        return Err("browser proxy body framing".into());
    }
    let mut body = vec![0; length];
    read_until(stream, &mut body, deadline)?;
    Ok((method.into(), path.into(), body))
}
fn respond(stream: &mut TcpStream, status: u16, body: &[u8]) -> Result<()> {
    write!(stream,"HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nAccess-Control-Allow-Private-Network: true\r\nCache-Control: no-store\r\n\r\n",body.len())?;
    stream.write_all(body)?;
    Ok(())
}
fn handle(mut stream: TcpStream, peer: &str) -> Result<()> {
    let request = read_request(&mut stream);
    match request {
        Ok((method, path, body)) => {
            if method == "OPTIONS" {
                return respond(&mut stream, 204, &[]);
            }
            match crate::body_join::http(peer, &method, &path, &body) {
                Ok((status, reply)) => respond(&mut stream, status, &reply),
                Err(_) => respond(&mut stream, 502, b"{\"error\":\"device-unavailable\"}"),
            }
        }
        Err(_) => respond(&mut stream, 400, b"{\"error\":\"invalid-join-request\"}"),
    }
}
pub fn run(peer: &str, port: u16) -> Result<()> {
    if port == 0 || peer.is_empty() || peer.len() > 255 || peer.contains(['/', '@', '\r', '\n']) {
        return Err("expected fixed device HOST:PORT and nonzero loopback port".into());
    }
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))?;
    eprintln!("Private join transport ready at http://127.0.0.1:{port}; device {peer}");
    for connection in listener.incoming() {
        let stream = connection?;
        if stream.peer_addr()?.ip().is_loopback() {
            let _ = handle(stream, peer);
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_are_exact_and_never_general_proxy() {
        assert!(route("POST", "/body/join"));
        assert!(route("GET", "/body/join/result?id=7"));
        for (method, path) in [
            ("GET", "/history"),
            ("POST", "http://peer/body/join"),
            ("GET", "/body/join/result?id=-1"),
            ("GET", "/body/join/result?id=7&peer=elsewhere"),
            ("POST", "/body/join/result?id=1"),
        ] {
            assert!(!route(method, path));
        }
    }
    fn roundtrip(request: Vec<u8>, peer: Option<String>) -> Vec<u8> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            handle(
                listener.accept().unwrap().0,
                peer.as_deref().unwrap_or("127.0.0.1:1"),
            )
            .unwrap()
        });
        let mut stream = TcpStream::connect(address).unwrap();
        stream.write_all(&request).unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).unwrap();
        server.join().unwrap();
        reply
    }
    #[test]
    fn oversized_browser_headers_forward_only_exact_public_proof() {
        let target = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = target.local_addr().unwrap().to_string();
        let device = std::thread::spawn(move || {
            let (mut socket, _) = target.accept().unwrap();
            let mut received = Vec::new();
            let mut byte = [0];
            while !received.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                received.push(byte[0]);
            }
            let header = String::from_utf8(received).unwrap();
            assert!(!header.contains("User-Agent"));
            assert!(!header.contains("Origin:"));
            let mut proof = [0; 3];
            socket.read_exact(&mut proof).unwrap();
            assert_eq!(&proof, b"abc");
            socket
                .write_all(b"HTTP/1.1 202 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .unwrap();
        });
        let request=format!("POST /body/join HTTP/1.1\r\nOrigin: null\r\nUser-Agent: {}\r\nContent-Length: 3\r\n\r\nabc","x".repeat(1200));
        let reply = String::from_utf8(roundtrip(request.into_bytes(), Some(peer))).unwrap();
        assert!(reply.starts_with("HTTP/1.1 202"));
        assert!(reply.contains("Access-Control-Allow-Origin: *"));
        device.join().unwrap();
    }
    #[test]
    fn preflight_and_malformed_framing_never_contact_device() {
        let reply = roundtrip(
            b"OPTIONS /body/join HTTP/1.1\r\nOrigin: null\r\n\r\n".to_vec(),
            None,
        );
        assert!(reply.starts_with(b"HTTP/1.1 204"));
        for request in [
            b"POST /body/join HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\n"
                .as_slice(),
            b"POST /body/join HTTP/1.1\r\nContent-Length: 1025\r\n\r\n",
            b"POST /body/join HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n",
        ] {
            assert!(roundtrip(request.to_vec(), None).starts_with(b"HTTP/1.1 400"));
        }
    }
}
