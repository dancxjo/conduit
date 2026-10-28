//! Bounded attended HTTP transport; response data grants no authority.
use super::*;
pub(crate) fn http(peer: &str, method: &str, path: &str, body: &[u8]) -> Result<(u16, Vec<u8>)> {
    http_content(peer, method, path, body, "application/json")
}
pub(crate) fn http_binary(peer: &str, path: &str, body: &[u8]) -> Result<(u16, Vec<u8>)> {
    http_content(peer, "POST", path, body, "application/octet-stream")
}
fn http_content(
    peer: &str,
    method: &str,
    path: &str,
    body: &[u8],
    content_type: &str,
) -> Result<(u16, Vec<u8>)> {
    use std::net::ToSocketAddrs;
    if peer.len() > 255 || peer.contains(['/', '@', '\r', '\n']) {
        return Err("join peer must be HOST:PORT".into());
    }
    let mut last = None;
    let deadline = Instant::now() + Duration::from_secs(3);
    for address in peer.to_socket_addrs()?.filter(|a| a.is_ipv4()).take(8) {
        let response = (|| -> Result<_> {
            let mut s = std::net::TcpStream::connect_timeout(
                &address,
                deadline
                    .checked_duration_since(Instant::now())
                    .ok_or("join HTTP deadline")?,
            )?;
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("join HTTP deadline")?;
            s.set_read_timeout(Some(remaining))?;
            s.set_write_timeout(Some(remaining))?;
            let request_header = format!("{method} {path} HTTP/1.1\r\nHost: {peer}\r\nConnection: close\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",body.len());
            if request_header.len() + body.len() > 1024 {
                return Err("join request exceeds native1024-byte frame".into());
            }
            s.write_all(request_header.as_bytes())?;
            s.write_all(body)?;
            let mut bytes = Vec::new();

            let mut block = [0; 1024];
            loop {
                s.set_read_timeout(Some(
                    deadline
                        .checked_duration_since(Instant::now())
                        .ok_or("join HTTP deadline")?,
                ))?;
                let n = s.read(&mut block)?;
                if n == 0 {
                    break;
                }
                if bytes.len() + n > 16384 {
                    return Err("join HTTP extent".into());
                }
                bytes.extend_from_slice(&block[..n]);
            }
            if bytes.len() > 16384 {
                return Err("join HTTP extent".into());
            }
            let at = bytes
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .ok_or("join HTTP framing")?;
            let header = std::str::from_utf8(&bytes[..at])?;
            let status = header
                .lines()
                .next()
                .and_then(|v| v.split_whitespace().nth(1))
                .ok_or("join HTTP status")?
                .parse()?;
            Ok((status, bytes[at + 4..].to_vec()))
        })();
        match response {
            Ok(r) => return Ok(r),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| "join peer has no bounded IPv4 route".into()))
}
