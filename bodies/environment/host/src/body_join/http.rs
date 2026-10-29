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
    // Resolution is synchronous and is not covered by the socket deadline.
    // Foreground schedulers use the resolved entrance below instead.
    for address in peer.to_socket_addrs()?.filter(|a| a.is_ipv4()).take(8) {
        match http_resolved_until(
            Request {
                peer,
                method,
                path,
                body,
                content_type,
            },
            address,
            deadline,
        ) {
            Ok(response) => return Ok(response),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| "join peer has no bounded IPv4 route".into()))
}

/// One already-resolved socket exchange under the caller's absolute deadline.
/// No DNS, route fallback, sleep or retry may extend this operation's budget.
/// A resolved address is transport selection only; callers verify response scope.
pub(crate) struct Request<'a> {
    pub(crate) peer: &'a str,
    pub(crate) method: &'a str,
    pub(crate) path: &'a str,
    pub(crate) body: &'a [u8],
    pub(crate) content_type: &'a str,
}
pub(crate) fn http_resolved_until(
    request: Request<'_>,
    address: std::net::SocketAddr,
    deadline: Instant,
) -> Result<(u16, Vec<u8>)> {
    let Request {
        peer,
        method,
        path,
        body,
        content_type,
    } = request;
    if !address.is_ipv4()
        || peer.is_empty()
        || peer.len() > 255
        || peer.contains(['/', '@', '\r', '\n'])
        || !matches!(method, "GET" | "POST")
        || !path.starts_with('/')
        || path.bytes().any(|b| b <= 32 || b == 127)
        || !matches!(
            content_type,
            "application/json" | "application/octet-stream"
        )
    {
        return Err("join HTTP resolved request scope".into());
    }
    let header = format!("{method} {path} HTTP/1.1\r\nHost: {peer}\r\nConnection: close\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n", body.len());
    if body.len() > 1024 || header.len() + body.len() > 1024 {
        return Err("join request exceeds native1024-byte frame".into());
    }
    let mut socket = std::net::TcpStream::connect_timeout(&address, remaining(deadline)?)?;
    write_until(&mut socket, header.as_bytes(), deadline)?;
    write_until(&mut socket, body, deadline)?;
    read_response(&mut socket, deadline)
}
fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| "join HTTP deadline".into())
}
fn write_until(
    socket: &mut std::net::TcpStream,
    mut bytes: &[u8],
    deadline: Instant,
) -> Result<()> {
    while !bytes.is_empty() {
        socket.set_write_timeout(Some(remaining(deadline)?))?;
        match socket.write(bytes) {
            Ok(0) => return Err("join HTTP write closed".into()),
            Ok(written) => bytes = &bytes[written..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    remaining(deadline)?;
    Ok(())
}

const MAXIMUM_RESPONSE_BYTES: usize = 16384;
struct ResponseHead {
    status: u16,
    body_at: usize,
    content_length: Option<usize>,
}
impl ResponseHead {
    fn complete(&self, received: usize, eof: bool) -> Result<bool> {
        match self.content_length {
            Some(length) => {
                let expected = self.body_at + length; // Checked during parsing.
                if received > expected {
                    return Err("join HTTP trailing bytes".into());
                }
                if received == expected {
                    return Ok(true);
                }
                if eof {
                    return Err("join HTTP truncated body".into());
                }
                Ok(false)
            }
            None => Ok(eof),
        }
    }
}
fn response_head(bytes: &[u8]) -> Result<Option<ResponseHead>> {
    let Some(at) = bytes.windows(4).position(|w| w == b"\r\n\r\n") else {
        return Ok(None);
    };
    let header = std::str::from_utf8(&bytes[..at])?;
    let mut lines = header.split("\r\n");
    let status_line = lines.next().ok_or("join HTTP status")?;
    let (version, rest) = status_line.split_once(' ').ok_or("join HTTP status")?;
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        || rest.len() < 3
        || !rest.as_bytes()[..3].iter().all(u8::is_ascii_digit)
        || (rest.len() > 3 && rest.as_bytes()[3] != b' ')
        || status_line.bytes().any(|b| b < 32 || b == 127)
    {
        return Err("join HTTP status".into());
    }
    let status = rest[..3].parse::<u16>()?;
    if !(200..=599).contains(&status) {
        return Err("join HTTP unsupported status".into());
    }
    let mut content_length = None;
    for line in lines {
        let (name, value) = line.split_once(':').ok_or("join HTTP header")?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
            || value.bytes().any(|b| (b < 32 && b != b'\t') || b == 127)
        {
            return Err("join HTTP header".into());
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err("join HTTP unsupported transfer encoding".into());
        }
        if name.eq_ignore_ascii_case("content-length") {
            let value = value.trim_matches([' ', '\t']);
            if content_length.is_some()
                || value.is_empty()
                || !value.bytes().all(|b| b.is_ascii_digit())
            {
                return Err("join HTTP content length".into());
            }
            content_length = Some(value.parse::<usize>()?);
        }
    }
    let body_at = at + 4;
    if body_at > MAXIMUM_RESPONSE_BYTES
        || content_length.is_some_and(|length| {
            body_at
                .checked_add(length)
                .is_none_or(|end| end > MAXIMUM_RESPONSE_BYTES)
        })
    {
        return Err("join HTTP extent".into());
    }
    Ok(Some(ResponseHead {
        status,
        body_at,
        content_length,
    }))
}
fn read_response(socket: &mut std::net::TcpStream, deadline: Instant) -> Result<(u16, Vec<u8>)> {
    let mut bytes = Vec::new();
    let mut head = None;
    let mut block = [0; 1024];
    loop {
        socket.set_read_timeout(Some(
            deadline
                .checked_duration_since(Instant::now())
                .ok_or("join HTTP deadline")?,
        ))?;
        let n = socket.read(&mut block)?;
        if Instant::now() >= deadline {
            return Err("join HTTP deadline".into());
        }
        if bytes.len() + n > MAXIMUM_RESPONSE_BYTES {
            return Err("join HTTP extent".into());
        }
        bytes.extend_from_slice(&block[..n]);
        if head.is_none() {
            head = response_head(&bytes)?;
        }
        if let Some(head) = head.as_ref() {
            if head.complete(bytes.len(), n == 0)? {
                // Content-Length delimits this response. Reject any already-read
                // surplus; close the one-use socket without waiting for EOF.
                return Ok((head.status, bytes.split_off(head.body_at)));
            }
        } else if n == 0 {
            return Err("join HTTP framing".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read_request(socket: &mut std::net::TcpStream) {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            assert_eq!(socket.read(&mut byte).unwrap(), 1);
            bytes.push(byte[0]);
            assert!(bytes.len() <= 1024);
        }
    }
    fn get_request() -> Request<'static> {
        Request {
            peer: "localhost:80",
            method: "GET",
            path: "/history",
            body: &[],
            content_type: "application/json",
        }
    }
    #[test]
    fn expired_resolved_operation_never_connects_or_resolves() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        assert!(http_resolved_until(
            get_request(),
            listener.local_addr().unwrap(),
            Instant::now()
        )
        .is_err());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        let mut invalid = get_request();
        invalid.path = "/history\r\nInjected: yes";
        assert!(http_resolved_until(
            invalid,
            listener.local_addr().unwrap(),
            Instant::now() + Duration::from_secs(1)
        )
        .is_err());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn resolved_operation_finishes_at_content_length_without_waiting_for_eof() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buffer = [0; 1024];
            read_request(&mut socket);
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc")
                .unwrap();
            // Keep the response side open until the client closes it.
            assert_eq!(socket.read(&mut buffer).unwrap(), 0);
        });
        let result = http_resolved_until(
            get_request(),
            address,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(result, (200, b"abc".to_vec()));
        server.join().unwrap();
    }
    #[test]
    fn slow_response_cannot_renew_the_callers_deadline_per_read() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            read_request(&mut socket);
            if socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n")
                .is_err()
            {
                return;
            }
            for _ in 0..100 {
                std::thread::sleep(Duration::from_millis(20));
                if socket.write_all(b"x").is_err() {
                    break;
                }
            }
        });
        let started = Instant::now();
        assert!(
            http_resolved_until(get_request(), address, started + Duration::from_millis(80))
                .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        server.join().unwrap();
    }
    #[test]
    fn response_framing_refuses_ambiguous_headers_and_extent() {
        for headers in [
            "Content-Length: ",
            "Content-Length: +1",
            "Content-Length: -1",
            "Content-Length: 1, 1",
            "Content-Length: 9999999999999999999999999999",
            "Content-Length: 1\r\nContent-Length: 1",
            "Content-Length: 1\r\ncOnTeNt-LeNgTh: 2",
            "Transfer-Encoding: chunked",
            "Transfer-Encoding: identity",
            "Content-Length: 1\r\nTransfer-Encoding: chunked",
            "Content-Length : 1",
            " Content-Length: 1",
            "Content-Length: 1\nIgnored: 2",
            "Content-Length: 1\rIgnored: 2",
            "Content-Length: 16384",
        ] {
            let bytes = format!("HTTP/1.1 200 OK\r\n{headers}\r\n\r\n");
            assert!(response_head(bytes.as_bytes()).is_err(), "{headers:?}");
        }
        for status in [
            "HTTP/2 200 OK",
            "HTTP/1.1 2000 OK",
            "HTTP/1.1 100 Continue",
            "HTTP/1.1 600 Invalid",
        ] {
            assert!(
                response_head(format!("{status}\r\nContent-Length: 0\r\n\r\n").as_bytes()).is_err()
            );
        }
    }
    #[test]
    fn exact_length_requires_all_body_bytes_and_refuses_observed_surplus() {
        let prefix = b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n";
        let head = response_head(prefix).unwrap().unwrap();
        assert!(!head.complete(prefix.len() + 2, false).unwrap());
        assert!(head.complete(prefix.len() + 2, true).is_err());
        assert!(head.complete(prefix.len() + 3, false).unwrap());
        assert!(head.complete(prefix.len() + 4, false).is_err());
        let empty = response_head(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n")
            .unwrap()
            .unwrap();
        assert!(empty.complete(empty.body_at, false).unwrap());
        let old = response_head(b"HTTP/1.0 200 OK\r\nConnection: close\r\n\r\n")
            .unwrap()
            .unwrap();
        assert!(!old.complete(old.body_at + 3, false).unwrap());
        assert!(old.complete(old.body_at + 3, true).unwrap());
    }
    #[test]
    fn body_extent_includes_headers_at_exact_bound() {
        let mut length = MAXIMUM_RESPONSE_BYTES;
        let header = loop {
            let header = format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\n\r\n");
            let available = MAXIMUM_RESPONSE_BYTES - header.len();
            if available == length {
                break header;
            }
            length = available;
        };
        for end in 0..header.len() {
            assert!(response_head(&header.as_bytes()[..end]).unwrap().is_none());
        }
        let head = response_head(header.as_bytes()).unwrap().unwrap();
        assert!(head.complete(MAXIMUM_RESPONSE_BYTES, false).unwrap());
        let oversized = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", length + 1);
        assert!(response_head(oversized.as_bytes()).is_err());
    }
}
