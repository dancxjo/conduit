//! Bounded exact read-only route; no request body or state-changing operation.
pub fn status(bytes: &[u8], capacity: usize) -> Option<u16> {
    let Some(end) = bytes.windows(4).position(|x| x == b"\r\n\r\n") else {
        return (bytes.len() >= capacity).then_some(431);
    };
    if end + 4 != bytes.len() {
        return Some(400);
    }
    let Some(line_end) = bytes.windows(2).position(|x| x == b"\r\n") else {
        return Some(400);
    };
    let line = &bytes[..line_end];
    if !line.starts_with(b"GET ") {
        return Some(405);
    }
    let Some(path) = line.strip_prefix(b"GET ").and_then(|x| {
        x.strip_suffix(b" HTTP/1.1")
            .or_else(|| x.strip_suffix(b" HTTP/1.0"))
    }) else {
        return Some(400);
    };
    if path != b"/environment" {
        return Some(404);
    }
    for header in bytes[line_end + 2..end + 2].split(|b| *b == b'\n') {
        let header = header.strip_suffix(b"\r").unwrap_or(header);
        if header.is_empty() {
            continue;
        }
        let Some(colon) = header.iter().position(|b| *b == b':') else {
            return Some(400);
        };
        let name = &header[..colon];
        if name.eq_ignore_ascii_case(b"content-length")
            || name.eq_ignore_ascii_case(b"transfer-encoding")
        {
            return Some(400);
        }
        if header.iter().any(|b| *b < 32 && *b != b'\t') {
            return Some(400);
        }
    }
    Some(200)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_route_and_finite_header_only() {
        assert_eq!(
            status(b"GET /environment HTTP/1.1\r\nHost: white\r\n\r\n", 512),
            Some(200)
        );
        assert_eq!(status(b"GET /environment HTTP/1.0\r\n\r\n", 512), Some(200));
        assert_eq!(status(b"GET /environment HTTP/1.1\r\n", 512), None);
        assert_eq!(status(&[b'x'; 512], 512), Some(431));
    }
    #[test]
    fn no_mutations_bodies_or_pipelined_requests() {
        assert_eq!(
            status(b"POST /environment HTTP/1.1\r\n\r\n", 512),
            Some(405)
        );
        assert_eq!(
            status(b"GET /environment?x HTTP/1.1\r\n\r\n", 512),
            Some(404)
        );
        for wire in [
            b"GET /environment HTTP/1.1\r\nContent-Length: 0\r\n\r\n".as_slice(),
            b"GET /environment HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n",
            b"GET /environment HTTP/1.1\r\n\r\nx",
            b"GET /environment HTTP/1.1\r\nBad\r\n\r\n",
        ] {
            assert_eq!(status(wire, 512), Some(400));
        }
    }
}
