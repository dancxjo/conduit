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
            if head_end
                .checked_add(content_length)
                .is_none_or(|total| total > MAX_HTTP_REQUEST_BYTES)
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
