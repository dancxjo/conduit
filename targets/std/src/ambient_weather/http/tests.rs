use super::*;
use std::{net::TcpListener, sync::mpsc, thread};

fn server(response: &'static [u8]) -> (SocketAddr, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let thread = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0u8; 1024];
        let mut used = 0;
        while !request[..used].windows(4).any(|v| v == b"\r\n\r\n") {
            let n = stream.read(&mut request[used..]).unwrap();
            assert!(n != 0);
            used += n;
        }
        assert_eq!(&request[..used], REQUEST);
        stream.write_all(response).unwrap();
    });
    (address, thread)
}

#[test]
fn exact_request_and_both_finite_response_frames_allocate_nothing_during_read() {
    for response in [
        &b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}"[..],
        &b"HTTP/1.0 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nConnection: close\r\n\r\n{}"[..],
    ] {
        let (endpoint, server) = server(response);
        let mut provider = OpenMeteoHttp::prepare(endpoint).unwrap();
        let mut output = [0u8; MAXIMUM_CURRENT_BODY_BYTES];
        let probe = crate::allocation_probe::begin();
        let result = provider.read_into(&conduit_time::encode_tick(0), &mut output,
            Instant::now() + MAXIMUM_WAIT, || false);
        let statistics = probe.finish_statistics();
        assert_eq!(result, Ok(2));
        assert_eq!(&output[..2], b"{}");
        assert_eq!(statistics.requested_bytes, 0);
        server.join().unwrap();
    }
}

#[test]
fn malformed_oversized_redirected_or_encoded_responses_refuse_without_output() {
    use OpenMeteoHttpRefusal as E;
    for (response, expected) in [
        (&b"HTTP/1.1 302 Found\r\nContent-Type: application/json\r\n\r\n{}"[..], E::Status(302)),
        (&b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n{}"[..], E::ContentType),
        (&b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 4097\r\n\r\n"[..], E::BodyBound),
        (&b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}"[..], E::Framing),
        (&b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"[..], E::Framing),
        (&b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Encoding: gzip\r\n\r\nxx"[..], E::Framing),
        (&b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 3\r\n\r\n{}"[..], E::Framing),
    ] {
        let (endpoint, server) = server(response);
        let mut provider = OpenMeteoHttp::prepare(endpoint).unwrap();
        let mut output = [0xA5; MAXIMUM_CURRENT_BODY_BYTES];
        let probe = crate::allocation_probe::begin();
        let result = provider.read_into(&conduit_time::encode_tick(0), &mut output,
            Instant::now() + MAXIMUM_WAIT, || false);
        let statistics = probe.finish_statistics();
        assert_eq!(result, Err(expected));
        assert_eq!(statistics.requested_bytes, 0);
        assert!(output.iter().all(|v| *v == 0xA5));
        server.join().unwrap();
    }
}

#[test]
fn cancellation_deadline_and_invalid_tick_do_not_publish() {
    use OpenMeteoHttpRefusal as E;
    let mut provider = OpenMeteoHttp::prepare("127.0.0.1:1".parse().unwrap()).unwrap();
    let tick = conduit_time::encode_tick(0);
    let mut output = [0xA5; MAXIMUM_CURRENT_BODY_BYTES];
    assert_eq!(
        provider.read_into(&tick, &mut output, Instant::now() + MAXIMUM_WAIT, || true),
        Err(E::Cancelled)
    );
    assert_eq!(
        provider.read_into(&tick, &mut output, Instant::now(), || false),
        Err(E::Deadline)
    );
    assert_eq!(
        provider.read_into(&[], &mut output, Instant::now() + MAXIMUM_WAIT, || false),
        Err(E::InvalidTick)
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    let (ready, started) = mpsc::sync_channel(0);
    let (finish, release) = mpsc::sync_channel(0);
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        ready.send(()).unwrap();
        release.recv().unwrap();
        drop(stream);
    });
    let mut provider = OpenMeteoHttp::prepare(endpoint).unwrap();
    let cancelled = std::cell::Cell::new(false);
    let probe = crate::allocation_probe::begin();
    let result = provider.read_into(&tick, &mut output, Instant::now() + MAXIMUM_WAIT, || {
        if !cancelled.get() && started.try_recv().is_ok() {
            cancelled.set(true);
        }
        cancelled.get()
    });
    let statistics = probe.finish_statistics();
    assert_eq!(result, Err(E::Cancelled));
    assert_eq!(statistics.requested_bytes, 0);
    assert!(output.iter().all(|v| *v == 0xA5));
    finish.send(()).unwrap();
    server.join().unwrap();
}

#[test]
#[ignore = "explicit live network diagnostic; no Base/Host Call/IMAGE or physical-display claim"]
fn live_seattle_http_acquisition_decodes_current_conditions() {
    use std::net::ToSocketAddrs;
    // DNS is preparation work outside the allocation-free read profile.
    let endpoint = ("api.open-meteo.com", 80)
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();
    let mut provider = OpenMeteoHttp::prepare(endpoint).unwrap();
    let mut output = [0u8; MAXIMUM_CURRENT_BODY_BYTES];
    let probe = crate::allocation_probe::begin();
    let result = provider.read_into(
        &conduit_time::encode_tick(0),
        &mut output,
        Instant::now() + MAXIMUM_WAIT,
        || false,
    );
    let statistics = probe.finish_statistics();
    let count = result.unwrap();
    assert_eq!(statistics.requested_bytes, 0);
    let weather = super::super::PreparedOpenMeteoWeather::prepare("Seattle", 900_000).unwrap();
    let reading = weather.decode_reading(&output[..count]).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    weather.classify(&reading, now).unwrap();
}
