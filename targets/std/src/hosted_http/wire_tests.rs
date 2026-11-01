use super::*;
fn request(method: HttpMethod, bytes: &[u8]) -> HttpRequest {
    HttpRequest {
        transaction_id: HttpTransactionId::new(1).unwrap(),
        method,
        target: HttpTarget::new(
            "sensor.invalid:8080".into(),
            "/reading".into(),
            conduit_web::HttpScheme::Http,
        )
        .unwrap(),
        headers: Default::default(),
        body: HttpBody::inline(bytes),
    }
}
#[test]
fn empty_read_requests_are_header_only_and_round_trip_without_body_framing() {
    for method in [HttpMethod::Get, HttpMethod::Head] {
        let request = request(method, &[]);
        let wire = encode_request(&request).unwrap();
        assert!(wire.ends_with(b"\r\n\r\n"));
        assert!(!wire
            .windows(b"Content-Length:".len())
            .any(|b| b == b"Content-Length:"));
        assert!(!wire
            .windows(b"Transfer-Encoding:".len())
            .any(|b| b == b"Transfer-Encoding:"));
        let decoded = read_request(&mut wire.as_slice(), request.transaction_id).unwrap();
        assert_eq!(decoded.method, method);
        assert_eq!(decoded.target, request.target);
        assert_eq!(decoded.body.as_inline(), Some([].as_slice()));
    }
}
#[test]
fn explicit_write_and_nonempty_read_content_keep_exact_length_and_payload() {
    for (method, body) in [
        (HttpMethod::Post, [].as_slice()),
        (HttpMethod::Post, b"abc".as_slice()),
        (HttpMethod::Get, b"abc".as_slice()),
    ] {
        let request = request(method, body);
        let wire = encode_request(&request).unwrap();
        let expected = format!("Content-Length: {}\r\n\r\n", body.len());
        assert!(wire
            .windows(expected.len())
            .any(|b| b == expected.as_bytes()));
        let decoded = read_request(&mut wire.as_slice(), request.transaction_id).unwrap();
        assert_eq!(decoded.method, method);
        assert_eq!(decoded.body.as_inline(), Some(body));
    }
}

#[test]
fn responses_still_require_finite_length_and_requests_reject_transfer_framing() {
    let id = HttpTransactionId::new(1).unwrap();
    assert!(read_response(&mut b"HTTP/1.1 200 OK\r\n\r\n".as_slice(), id.clone()).is_err());
    assert!(read_request(
        &mut b"GET / HTTP/1.1\r\nHost: sensor.invalid\r\nTransfer-Encoding: chunked\r\n\r\n"
            .as_slice(),
        id,
    )
    .is_err());
}
