//! Finite HTTP framing for the loopback Patchbay renderer adapter.

use super::{ServerError, MAX_HTTP_REQUEST_BYTES};
use std::io::{Read, Write};
use std::net::TcpStream;

pub(super) struct HttpRequest {
    pub(super) head: String,
    pub(super) body: Vec<u8>,
}

pub(super) fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), ServerError> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'self'; script-src 'self' blob: 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self' ws://127.0.0.1:*; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

pub(super) fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, ServerError> {
    let mut bytes = Vec::with_capacity(1024);
    let mut chunk = [0u8; 512];
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_HTTP_REQUEST_BYTES {
            return Err(ServerError::RequestTooLarge);
        }
        if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let head_end = header_end + 4;
            let head = std::str::from_utf8(&bytes[..head_end])
                .map_err(|_| ServerError::InvalidRequest)?
                .to_owned();
            let content_length = head
                .lines()
                .find_map(|line| {
                    line.strip_prefix("Content-Length: ")
                        .or_else(|| line.strip_prefix("content-length: "))
                })
                .map(|value| value.parse::<usize>())
                .transpose()
                .map_err(|_| ServerError::InvalidRequest)?
                .unwrap_or(0);
            let request_bound =
                if head.starts_with("POST /api/body-host-planning-offer HTTP/1.1\r\n") {
                    super::body_host_planning_offer::MAX_BODY_HOST_PLANNING_OFFER_BYTES + 8 * 1024
                } else if head.starts_with("POST /api/body-face-acknowledge HTTP/1.1\r\n")
                    || head.starts_with("POST /api/body-face-interact HTTP/1.1\r\n")
                {
                    1024 * 1024 + 8 * 1024
                } else {
                    MAX_HTTP_REQUEST_BYTES
                };
            if head_end
                .checked_add(content_length)
                .is_none_or(|total| total > request_bound)
            {
                return Err(ServerError::RequestTooLarge);
            }
            while bytes.len() < head_end + content_length {
                let count = stream.read(&mut chunk)?;
                if count == 0 {
                    return Err(ServerError::InvalidRequest);
                }
                bytes.extend_from_slice(&chunk[..count]);
            }
            return Ok(HttpRequest {
                head,
                body: bytes[head_end..head_end + content_length].to_vec(),
            });
        }
    }
    Err(ServerError::InvalidRequest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn receive(path: &str, length: usize, send_body: bool) -> Result<HttpRequest, ServerError> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let head = format!("POST {path} HTTP/1.1\r\nContent-Length: {length}\r\n\r\n");
        let writer = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream.write_all(head.as_bytes()).unwrap();
            if send_body {
                let _ = stream.write_all(&vec![b'x'; length]);
            }
        });
        let (mut stream, _) = listener.accept().unwrap();
        let result = read_request(&mut stream);
        writer.join().unwrap();
        result
    }

    #[test]
    fn complete_planning_control_disclosure_crosses_http_without_expanding_workspace_requests() {
        let length = 427_250;
        let request = receive("/api/body-host-planning-offer", length, true).unwrap();
        assert_eq!(request.body.len(), length);
        assert!(matches!(
            receive("/api/workspace", length, false),
            Err(ServerError::RequestTooLarge)
        ));
        assert!(matches!(
            receive(
                "/api/body-host-planning-offer",
                super::super::body_host_planning_offer::MAX_BODY_HOST_PLANNING_OFFER_BYTES
                    + 8 * 1024
                    + 1,
                false
            ),
            Err(ServerError::RequestTooLarge)
        ));
    }
}
