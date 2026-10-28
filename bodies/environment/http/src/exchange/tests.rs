//! Deterministic async transport fixture; parser, response guard and mailbox are real.
use super::*;
use crate::tests::allocation_probe;
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
#[derive(Debug)]
struct IoError;
impl core::fmt::Display for IoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("fixture I/O failure")
    }
}
impl core::error::Error for IoError {}
impl embedded_io_async::Error for IoError {
    fn kind(&self) -> embedded_io_async::ErrorKind {
        embedded_io_async::ErrorKind::Other
    }
}
struct Socket<'a> {
    input: &'a [u8],
    output: [u8; 4096],
    written: usize,
    write_failure: bool,
    flush_failure: bool,
    pending_flush: bool,
}
impl<'a> Socket<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            output: [0; 4096],
            written: 0,
            write_failure: false,
            flush_failure: false,
            pending_flush: false,
        }
    }
}
impl embedded_io_async::ErrorType for Socket<'_> {
    type Error = IoError;
}
impl Read for Socket<'_> {
    async fn read(&mut self, output: &mut [u8]) -> Result<usize, IoError> {
        let length = self.input.len().min(output.len()).min(3);
        output[..length].copy_from_slice(&self.input[..length]);
        self.input = &self.input[length..];
        Ok(length)
    }
}
impl Write for Socket<'_> {
    async fn write(&mut self, input: &[u8]) -> Result<usize, IoError> {
        if self.write_failure {
            return Ok(0);
        }
        let length = input.len().min(7);
        self.output[self.written..self.written + length].copy_from_slice(&input[..length]);
        self.written += length;
        Ok(length)
    }
    async fn flush(&mut self) -> Result<(), IoError> {
        if self.pending_flush {
            core::future::pending::<()>().await;
        }
        if self.flush_failure {
            Err(IoError)
        } else {
            Ok(())
        }
    }
}
fn limits() -> NetworkSetupLimits {
    NetworkSetupLimits {
        minimum_credential_bytes: 8,
        maximum_credential_bytes: 64,
    }
}
fn poll<T>(future: impl Future<Output = T>) -> Poll<T> {
    let mut future = core::pin::pin!(future);
    future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
}
fn post() -> heapless::String<256> {
    use core::fmt::Write;
    let body = "ssid=Fixture+LAN&password=fixture-password";
    let mut request = heapless::String::new();
    write!(request, "POST /network HTTP/1.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
    request
}
#[test]
fn actual_stream_exchange_acknowledges_only_after_successful_flush() {
    let request = post();
    for mode in 0..4 {
        let mut mailbox = NetworkSetupMailbox::new(1);
        let mut socket = Socket::new(request.as_bytes());
        socket.write_failure = mode == 1;
        socket.flush_failure = mode == 2;
        socket.pending_flush = mode == 3;
        let ((), allocation) = allocation_probe::observe(|| {
            let route = NetworkSetupRoute::new(&mut mailbox, limits()).unwrap();
            let result = poll(exchange(&mut socket, || 0, Some(route)));
            match mode {
                0 => assert_eq!(
                    result,
                    Poll::Ready(Ok(ExchangeOutcome::SubmissionAcknowledged))
                ),
                1 => assert_eq!(result, Poll::Ready(Err(ExchangeRefusal::Closed))),
                2 => assert_eq!(result, Poll::Ready(Err(ExchangeRefusal::Transport))),
                _ => assert_eq!(result, Poll::Pending),
            }
            let mut output = [0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
            let result = mailbox.take_acknowledged(&mut output);
            if mode == 0 {
                let request =
                    conduit_net::decode_network_join_request(&output[..result.unwrap()]).unwrap();
                assert_eq!(request.ssid, b"Fixture LAN");
                assert_eq!(request.credential, b"fixture-password");
                assert_eq!(
                    mailbox.take_acknowledged(&mut output),
                    Err(NetworkSetupRefusal::NoSubmission)
                );
            } else {
                assert_eq!(result, Err(NetworkSetupRefusal::NoSubmission));
            }
            output.fill(0);
        });
        assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
        assert!(!socket.output[..socket.written]
            .windows(b"fixture-password".len())
            .any(|s| s == b"fixture-password"));
    }
}
#[test]
fn setup_route_requires_root_installation_and_page_never_claims_join_success() {
    let request = b"GET /network HTTP/1.1\r\nHost: device\r\n\r\n";
    let mut socket = Socket::new(request);
    assert_eq!(
        poll(exchange(&mut socket, || 0, None)),
        Poll::Ready(Ok(ExchangeOutcome::NotFound))
    );
    let mut mailbox = NetworkSetupMailbox::new(1);
    let mut socket = Socket::new(request);
    let route = NetworkSetupRoute::new(&mut mailbox, limits()).unwrap();
    assert_eq!(
        poll(exchange(&mut socket, || 0, Some(route))),
        Poll::Ready(Ok(ExchangeOutcome::NetworkPage))
    );
    assert!(core::str::from_utf8(&socket.output[..socket.written])
        .unwrap()
        .contains("Connect to Wi-Fi"));
    assert!(!mailbox.has_acknowledged());
}

#[test]
fn preparation_route_has_no_face_or_join_claim_and_retains_private_input_after_flush() {
    let mut mailbox = NetworkSetupMailbox::new(1);
    for request in [
        b"GET / HTTP/1.1\r\n\r\n".as_slice(),
        b"GET /reading HTTP/1.1\r\n\r\n",
    ] {
        let mut socket = Socket::new(request);
        let route = NetworkSetupRoute::preparation_only(&mut mailbox, limits()).unwrap();
        assert_eq!(
            poll(exchange(&mut socket, || 0, Some(route))),
            Poll::Ready(Ok(ExchangeOutcome::ReadingUnavailable))
        );
        assert!(socket.output[..socket.written].starts_with(b"HTTP/1.1 503"));
    }
    let mut socket = Socket::new(b"GET /network HTTP/1.1\r\n\r\n");
    let route = NetworkSetupRoute::preparation_only(&mut mailbox, limits()).unwrap();
    assert_eq!(
        poll(exchange(&mut socket, || 0, Some(route))),
        Poll::Ready(Ok(ExchangeOutcome::NetworkPage))
    );
    assert!(core::str::from_utf8(&socket.output[..socket.written])
        .unwrap()
        .contains("does not start joining"));
    let request = post();
    let mut socket = Socket::new(request.as_bytes());
    let ((), allocation) = allocation_probe::observe(|| {
        let route = NetworkSetupRoute::preparation_only(&mut mailbox, limits()).unwrap();
        assert_eq!(
            poll(exchange(&mut socket, || 0, Some(route))),
            Poll::Ready(Ok(ExchangeOutcome::SubmissionAcknowledged))
        );
        let prepared = mailbox
            .prepare_acknowledged(|request| (request.ssid.len(), request.credential.len()))
            .unwrap();
        assert_eq!(prepared, (11, 16));
        assert!(mailbox.has_acknowledged());
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let response = core::str::from_utf8(&socket.output[..socket.written]).unwrap();
    assert!(response.contains("No network join has started"));
    assert!(!response.contains("fixture-password"));
    assert!(!response.contains("Fixture LAN"));
}

#[test]
fn shared_socket_custody_blocks_source_until_exchange_flushes_or_cancels() {
    let request = post();
    for pending_flush in [false, true] {
        let mut channel = NetworkSetupChannel::new(1);
        let (writer, mut reader) = channel.split();
        let mut socket = Socket::new(request.as_bytes());
        socket.pending_flush = pending_flush;
        let ((), allocation) = allocation_probe::observe(|| {
            {
                let mut work = core::pin::pin!(async {
                    let mut mailbox = writer.lock().await;
                    let route = NetworkSetupRoute::new(&mut mailbox, limits()).unwrap();
                    exchange(&mut socket, || 0, Some(route)).await
                });
                let result = work.as_mut().poll(&mut Context::from_waker(Waker::noop()));
                if pending_flush {
                    assert_eq!(result, Poll::Pending);
                    assert_eq!(
                        reader.has_acknowledged(),
                        Err(NetworkSetupReadRefusal::Busy)
                    );
                } else {
                    assert_eq!(
                        result,
                        Poll::Ready(Ok(ExchangeOutcome::SubmissionAcknowledged))
                    );
                }
                // Drop cancels a stalled flush and releases actual socket custody.
            }
            assert_eq!(reader.has_acknowledged(), Ok(!pending_flush));
            let mut output = [0; 167];
            let result = reader.take_acknowledged(&mut output);
            if pending_flush {
                assert_eq!(
                    result,
                    Err(NetworkSetupReadRefusal::Input(
                        NetworkSetupRefusal::NoSubmission
                    ))
                );
            } else {
                let request =
                    conduit_net::decode_network_join_request(&output[..result.unwrap()]).unwrap();
                assert_eq!(request.ssid, b"Fixture LAN");
                assert_eq!(request.credential, b"fixture-password");
            }
            output.fill(0);
        });
        assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    }
}

#[test]
fn bootstrap_control_offers_joining_without_inventing_an_environmental_face() {
    let mut mailbox = NetworkSetupMailbox::new(1);
    let mut reading = Socket::new(b"GET /reading HTTP/1.1\r\n\r\n");
    let route = NetworkSetupRoute::bootstrap_control(&mut mailbox, limits()).unwrap();
    assert_eq!(
        poll(exchange(&mut reading, || 0, Some(route))),
        Poll::Ready(Ok(ExchangeOutcome::ReadingUnavailable))
    );
    let mut page = Socket::new(b"GET /network HTTP/1.1\r\n\r\n");
    let route = NetworkSetupRoute::bootstrap_control(&mut mailbox, limits()).unwrap();
    assert_eq!(
        poll(exchange(&mut page, || 0, Some(route))),
        Poll::Ready(Ok(ExchangeOutcome::NetworkPage))
    );
    assert!(core::str::from_utf8(&page.output[..page.written])
        .unwrap()
        .contains("Connect to Wi-Fi"));
    let post = post();
    let mut socket = Socket::new(post.as_bytes());
    let route = NetworkSetupRoute::bootstrap_control(&mut mailbox, limits()).unwrap();
    assert_eq!(
        poll(exchange(&mut socket, || 0, Some(route))),
        Poll::Ready(Ok(ExchangeOutcome::SubmissionAcknowledged))
    );
    assert!(mailbox.has_acknowledged());
    assert!(!core::str::from_utf8(&socket.output[..socket.written])
        .unwrap()
        .contains("fixture-password"));
}

#[test]
fn supplied_mask_reads_one_exact_face_without_mutation_or_play_allocation() {
    use conduit_core::Quantity;
    use conduit_environment_face::{PreparedObservationFace, SensorStatus};
    use conduit_face::FaceBasis;
    let mut owner = PreparedObservationFace::prepare(
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/explicit-reading".into()),
            checked_plot_id: Some("checked/explicit-reading".into()),
            expanded_plot_id: Some("expanded/explicit-reading".into()),
            plan_id: Some("plan/explicit-reading".into()),
            active_play_id: Some("play/explicit-reading".into()),
            sign_ids: alloc::vec![],
        },
        2,
        30000,
    )
    .unwrap();
    owner
        .publish(
            Some((
                Quantity::parse_plot_literal("19°C").unwrap(),
                Quantity::parse_plot_literal("55%").unwrap(),
            )),
            SensorStatus::Available,
            1000,
        )
        .unwrap();
    let face = owner.current();
    let before = face.clone();
    for (request, json) in [
        (b"GET / HTTP/1.1\r\n\r\n".as_slice(), false),
        (b"GET /reading HTTP/1.1\r\n\r\n".as_slice(), true),
    ] {
        let expected = crate::face_response(face, true, 1100, json).unwrap();
        let mut socket = Socket::new(request);
        let mut calls = 0;
        let ((), memory) = allocation_probe::observe(|| {
            assert_eq!(
                poll(exchange_with_reading(
                    &mut socket,
                    || 1100,
                    None,
                    |now, selected| {
                        calls += 1;
                        assert_eq!((now, selected), (1100, json));
                        crate::face_response(face, true, now, selected).ok()
                    }
                )),
                Poll::Ready(Ok(ExchangeOutcome::Reading))
            );
            assert_eq!(calls, 1); // Partial writes do not re-read or re-render.
            assert_eq!(&socket.output[..socket.written], expected.as_bytes());
            assert_eq!(face, &before);
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
}

#[test]
fn explicit_reader_cannot_override_bootstrap_and_absence_returns_unavailable() {
    for bootstrap in [false, true] {
        let mut mailbox = NetworkSetupMailbox::new(1);
        let mut socket = Socket::new(b"GET /reading HTTP/1.1\r\n\r\n");
        let route = if bootstrap {
            NetworkSetupRoute::bootstrap_control(&mut mailbox, limits())
        } else {
            NetworkSetupRoute::new(&mut mailbox, limits())
        }
        .unwrap();
        let mut calls = 0;
        let ((), memory) = allocation_probe::observe(|| {
            assert_eq!(
                poll(exchange_with_reading(
                    &mut socket,
                    || 0,
                    Some(route),
                    |_, _| {
                        calls += 1;
                        None
                    }
                )),
                Poll::Ready(Ok(ExchangeOutcome::ReadingUnavailable))
            );
            assert_eq!(calls, usize::from(!bootstrap));
            assert!(socket.output[..socket.written].starts_with(b"HTTP/1.1 503"));
        });
        assert!(!mailbox.has_acknowledged());
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
}

#[test]
fn retained_response_backing_is_reused_without_leaking_a_previous_reading() {
    let mut response = heapless::String::new();
    for available in [true, false] {
        let mut socket = Socket::new(b"GET /reading HTTP/1.1\r\n\r\n");
        let ((), memory) = allocation_probe::observe(|| {
            let result = poll(exchange_buffered(
                &mut socket,
                || 42,
                None,
                &mut response,
                |now, json| {
                    assert_eq!(now, 42);
                    assert!(json);
                    available.then(|| {
                        heapless::String::try_from("HTTP/1.1 200 OK\r\n\r\nfixture").unwrap()
                    })
                },
            ));
            assert_eq!(
                result,
                Poll::Ready(Ok(if available {
                    ExchangeOutcome::Reading
                } else {
                    ExchangeOutcome::ReadingUnavailable
                }))
            );
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        if available {
            assert_eq!(&socket.output[..socket.written], response.as_bytes());
        } else {
            assert!(response.is_empty());
            assert!(socket.output[..socket.written].starts_with(b"HTTP/1.1 503"));
        }
    }
}

#[test]
fn history_is_explicitly_installed_and_bootstrap_cannot_export_it() {
    for bootstrap in [false, true] {
        let mut socket = Socket::new(b"GET /history?after=37 HTTP/1.1\r\n\r\n");
        let mut mailbox = NetworkSetupMailbox::new(1);
        let route = if bootstrap {
            NetworkSetupRoute::bootstrap_control(&mut mailbox, limits())
        } else {
            NetworkSetupRoute::new(&mut mailbox, limits())
        }
        .unwrap();
        let mut output = heapless::String::new();
        let mut called = false;
        let outcome = poll(exchange_buffered_with_history(
            &mut socket,
            || 0,
            Some(route),
            &mut output,
            |_, _| None,
            |after| {
                assert_eq!(after, 37);
                called = true;
                Some(heapless::String::try_from("HTTP/1.0 200 OK\r\n\r\nfixture").unwrap())
            },
        ));
        assert_eq!(called, !bootstrap);
        assert!(
            matches!(outcome, Poll::Ready(Ok(value)) if value == if bootstrap { ExchangeOutcome::ReadingUnavailable } else { ExchangeOutcome::History })
        );
    }
    for request in [
        &b"GET /history?after=01 HTTP/1.1\r\n\r\n"[..],
        &b"GET /history?after=1&extra=2 HTTP/1.1\r\n\r\n"[..],
        &b"GET /history?after=4294967296 HTTP/1.1\r\n\r\n"[..],
        &b"GET /history?after= HTTP/1.1\r\n\r\n"[..],
        &b"GET /history?after=-1 HTTP/1.1\r\n\r\n"[..],
    ] {
        let mut socket = Socket::new(request);
        let outcome = poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None));
        assert!(matches!(
            outcome,
            Poll::Ready(Err(ExchangeRefusal::Framing(_)))
        ));
    }
    let mut socket = Socket::new(b"GET /history?after=0 HTTP/1.1\r\n\r\n");
    assert!(matches!(
        poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
        Poll::Ready(Ok(ExchangeOutcome::ReadingUnavailable))
    ));
}

#[test]
fn body_join_transport_is_explicit_bounded_and_serves_only_queued_public_material() {
    use crate::body_join::{self, Action, Reply};
    let mut unavailable = Socket::new(b"GET /body/join HTTP/1.1\r\n\r\n");
    assert_eq!(
        poll(exchange_with_reading(
            &mut unavailable,
            || 0,
            None,
            |_, _| None
        )),
        Poll::Ready(Ok(ExchangeOutcome::BodyJoin))
    );
    assert!(
        core::str::from_utf8(&unavailable.output[..unavailable.written])
            .unwrap()
            .starts_with("HTTP/1.1 503")
    );
    body_join::install_provider(|action| match action {
        Action::Submit(bytes) => {
            assert_eq!(bytes, b"{}");
            Reply::Queued(7)
        }
        Action::Result(7) => {
            Reply::Receipt(alloc::vec::Vec::from(&b"{\"signed_fixture\":true}"[..]))
        }
        Action::Result(_) => Reply::Unknown,
    })
    .unwrap();
    for (request,status,contains) in [
        (&b"POST /body/join HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}"[..],"202 Accepted","body-join-queued@1"),
        (&b"GET /body/join/result?id=7 HTTP/1.1\r\n\r\n"[..],"200 OK","signed_fixture"),
        (&b"OPTIONS /body/join HTTP/1.1\r\nOrigin: null\r\nAccess-Control-Request-Method: POST\r\n\r\n"[..],"204 No Content","Access-Control-Allow-Headers: Content-Type"),
        (&b"GET /body/join HTTP/1.1\r\n\r\n"[..],"200 OK","private USB invitation"),
    ] {
        let mut socket=Socket::new(request);
        assert_eq!(poll(exchange_with_reading(&mut socket,||0,None,|_,_|None)),Poll::Ready(Ok(ExchangeOutcome::BodyJoin)));
        let output=core::str::from_utf8(&socket.output[..socket.written]).unwrap();assert!(output.contains(status));assert!(output.contains(contains));assert!(!output.contains("invitation.secret"));
    }
    for request in [
        &b"GET /body/join/result?id=07 HTTP/1.1\r\n\r\n"[..],
        &b"POST /body/join HTTP/1.1\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\n{}"[..],
    ] {
        let mut socket = Socket::new(request);
        assert!(matches!(
            poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
            Poll::Ready(Err(_))
        ));
    }
}

#[test]
fn acknowledged_input_remains_private_until_outer_connection_custody_is_retired() {
    let request = post();
    let mut channel = NetworkSetupChannel::new(1);
    let (writer, mut reader) = channel.split();
    let mut socket = Socket::new(request.as_bytes());
    let Poll::Ready(mut custody) = poll(writer.lock()) else {
        panic!("fresh channel must lock")
    };
    let route = NetworkSetupRoute::new(&mut custody, limits()).unwrap();
    assert_eq!(
        poll(exchange(&mut socket, || 0, Some(route))),
        Poll::Ready(Ok(ExchangeOutcome::SubmissionAcknowledged))
    );
    // The exchange is complete and its response is acknowledged, but the real
    // outer connection owner retains custody across its separate close flush.
    assert_eq!(
        reader.has_acknowledged(),
        Err(NetworkSetupReadRefusal::Busy)
    );
    socket.pending_flush = true;
    {
        let mut closing = core::pin::pin!(socket.flush());
        assert!(closing
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
        assert_eq!(
            reader.prepare_acknowledged(|_| ()),
            Err(NetworkSetupReadRefusal::Busy)
        );
        assert_eq!(
            reader.take_acknowledged(&mut [0; 167]),
            Err(NetworkSetupReadRefusal::Busy)
        );
        // A bounded close timeout drops this pending operation. It does not
        // itself release the still-retained outer mailbox custody.
    }
    assert_eq!(
        reader.has_acknowledged(),
        Err(NetworkSetupReadRefusal::Busy)
    );
    drop(custody);
    assert_eq!(reader.has_acknowledged(), Ok(true));
    let mut output = [0; 167];
    let length = reader.take_acknowledged(&mut output).unwrap();
    assert_eq!(
        conduit_net::decode_network_join_request(&output[..length])
            .unwrap()
            .ssid,
        b"Fixture LAN"
    );
    output.fill(0);
}

#[test]
fn custody_offer_route_is_bounded_and_never_an_erase_request() {
    use crate::observed_custody::{self, Reply};
    let mut unavailable = Socket::new(b"GET /history/custody/offer?slot=0 HTTP/1.1\r\n\r\n");
    assert_eq!(
        poll(exchange_with_reading(
            &mut unavailable,
            || 0,
            None,
            |_, _| None
        )),
        Poll::Ready(Ok(ExchangeOutcome::ObservedOffer))
    );
    assert!(
        core::str::from_utf8(&unavailable.output[..unavailable.written])
            .unwrap()
            .contains("503")
    );
    observed_custody::install_provider(|slot| match slot {
        0 => Reply::Pending,
        1 => Reply::Offer(alloc::vec::Vec::from(
            &b"{\"signed_offer_fixture\":true}"[..],
        )),
        _ => Reply::Refused,
    })
    .unwrap();
    for (request, status) in [
        (
            &b"GET /history/custody/offer?slot=0 HTTP/1.1\r\n\r\n"[..],
            "202 Accepted",
        ),
        (
            &b"GET /history/custody/offer?slot=1 HTTP/1.0\r\n\r\n"[..],
            "200 OK",
        ),
        (
            &b"GET /history/custody/offer?slot=5 HTTP/1.1\r\n\r\n"[..],
            "409 Conflict",
        ),
    ] {
        let mut socket = Socket::new(request);
        assert_eq!(
            poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
            Poll::Ready(Ok(ExchangeOutcome::ObservedOffer))
        );
        assert!(core::str::from_utf8(&socket.output[..socket.written])
            .unwrap()
            .contains(status));
    }
    for request in [
        &b"GET /history/custody/offer?slot=01 HTTP/1.1\r\n\r\n"[..],
        &b"GET /history/custody/offer?slot=6 HTTP/1.1\r\n\r\n"[..],
        &b"POST /history/custody/offer?slot=1 HTTP/1.1\r\nContent-Length: 0\r\n\r\n"[..],
    ] {
        let mut socket = Socket::new(request);
        assert!(!matches!(
            poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
            Poll::Ready(Ok(ExchangeOutcome::ObservedOffer))
        ));
    }
}

#[test]
fn custody_ack_transport_bounds_framing_and_keeps_bootstrap_out_of_provider() {
    use crate::observed_custody::{self, Reply};
    fn request(slot: &str, headers: &str, body: &str) -> alloc::string::String {
        alloc::format!("POST /history/custody/ack?slot={slot} HTTP/1.1\r\n{headers}\r\n{body}")
    }
    let valid_headers = "Content-Type: application/json\r\nContent-Length: 2\r\n";
    let valid = request("0", valid_headers, "{}");
    let mut socket = Socket::new(valid.as_bytes());
    assert!(!observed_custody::ack_available());
    assert_eq!(
        poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
        Poll::Ready(Ok(ExchangeOutcome::ObservedAck))
    );
    assert!(core::str::from_utf8(&socket.output[..socket.written])
        .unwrap()
        .contains("503 Service Unavailable"));
    observed_custody::install_ack_provider(|slot, bytes| {
        assert_eq!(bytes, b"{}");
        match slot {
            0 => Reply::Pending,
            1 => Reply::Offer(alloc::vec::Vec::from(&b"{\"verified_receipt\":true}"[..])),
            _ => Reply::Refused,
        }
    })
    .unwrap();
    assert!(observed_custody::ack_available());
    assert!(observed_custody::install_ack_provider(|_, _| Reply::Pending).is_err());
    for (slot, status) in [
        ("0", "202 Accepted"),
        ("1", "200 OK"),
        ("5", "409 Conflict"),
    ] {
        let wire = request(slot, valid_headers, "{}");
        let mut socket = Socket::new(wire.as_bytes());
        assert_eq!(
            poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
            Poll::Ready(Ok(ExchangeOutcome::ObservedAck))
        );
        let response = core::str::from_utf8(&socket.output[..socket.written]).unwrap();
        assert!(response.contains(status));
        assert!(response.contains("Cache-Control: no-store"));
    }
    for headers in [
        "Content-Type: text/plain\r\nContent-Length: 2\r\n",
        "Content-Type: application/json; charset=utf-8\r\nContent-Length: 2\r\n",
        "Content-Type: application/json\r\nContent-Type: application/json\r\nContent-Length: 2\r\n",
        "Content-Type: application/json\r\nContent-Length: 2\r\nContent-Length: 2\r\n",
        "Content-Type: application/json\r\nTransfer-Encoding: chunked\r\nContent-Length: 2\r\n",
        "Content-Type: application/json\r\n",
    ] {
        let wire = request("0", headers, "{}");
        let mut socket = Socket::new(wire.as_bytes());
        assert!(matches!(
            poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
            Poll::Ready(Err(ExchangeRefusal::Framing(_)))
        ));
    }
    for slot in ["", "01", "6", "-1", "1&extra=2"] {
        let wire = request(slot, valid_headers, "{}");
        let mut socket = Socket::new(wire.as_bytes());
        assert!(matches!(
            poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
            Poll::Ready(Err(ExchangeRefusal::Framing(_)))
        ));
    }
    for length in [0, 769] {
        let body = "x".repeat(length);
        let headers =
            alloc::format!("Content-Type: application/json\r\nContent-Length: {length}\r\n");
        let wire = request("0", &headers, &body);
        let mut socket = Socket::new(wire.as_bytes());
        assert!(matches!(
            poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
            Poll::Ready(Err(ExchangeRefusal::Framing(_)))
        ));
    }
    // Parsing is bounded data transport, not JSON/signature verification. The
    // Root callback may refuse malformed or unauthenticated JSON independently.
    for bytes in [alloc::vec::Vec::new(), alloc::vec![0; 769]] {
        assert!(matches!(
            observed_custody::dispatch_ack(0, bytes),
            Reply::Refused
        ));
    }
    assert!(matches!(
        observed_custody::dispatch_ack(6, alloc::vec![1]),
        Reply::Refused
    ));
    // A different body would panic in the installed provider: bootstrap must
    // refuse without invoking it, for either preparation or control admission.
    for preparation in [false, true] {
        let mut mailbox = NetworkSetupMailbox::new(1);
        let route = if preparation {
            NetworkSetupRoute::preparation_only(&mut mailbox, limits())
        } else {
            NetworkSetupRoute::bootstrap_control(&mut mailbox, limits())
        }
        .unwrap();
        let wire = request(
            "0",
            "Content-Type: application/json\r\nContent-Length: 1\r\n",
            "x",
        );
        let mut socket = Socket::new(wire.as_bytes());
        assert_eq!(
            poll(exchange_with_reading(
                &mut socket,
                || 0,
                Some(route),
                |_, _| None
            )),
            Poll::Ready(Ok(ExchangeOutcome::ObservedAck))
        );
        assert!(core::str::from_utf8(&socket.output[..socket.written])
            .unwrap()
            .contains("503 Service Unavailable"));
    }
}

#[test]
fn custody_ack_exact_limit_uses_existing_request_buffer_and_waits_for_complete_body() {
    let body = "x".repeat(crate::observed_custody::MAXIMUM_ACK_BYTES);
    let headers = alloc::format!("POST /history/custody/ack?slot=5 HTTP/1.0\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len());
    assert!(headers.len() + body.len() <= 1024);
    let mut request = NetworkRequestBuffer::for_http();
    request.append(headers.as_bytes()).unwrap();
    request.append(&body.as_bytes()[..767]).unwrap();
    assert!(matches!(request.take(limits()), Ok(None)));
    request.append(&body.as_bytes()[767..]).unwrap();
    assert!(
        matches!(request.take(limits()), Ok(Some(NetworkHttpRequest::ObservedAck { slot: 5, bytes })) if bytes == body.as_bytes())
    );
    assert!(matches!(
        request.take(limits()),
        Err(NetworkHttpRefusal::Closed)
    ));

    for method in ["GET", "PUT", "DELETE"] {
        let wire = alloc::format!(
            "{method} /history/custody/ack?slot=0 HTTP/1.1\r\nContent-Length: 0\r\n\r\n"
        );
        let mut request = NetworkRequestBuffer::for_http();
        request.append(wire.as_bytes()).unwrap();
        assert!(matches!(
            request.take(limits()),
            Err(NetworkHttpRefusal::UnsupportedRoute)
        ));
    }
    // Extra bytes in the same framed message are never accepted as an ACK.
    let wire = b"POST /history/custody/ack?slot=0 HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}x";
    let mut request = NetworkRequestBuffer::for_http();
    request.append(wire).unwrap();
    assert!(matches!(
        request.take(limits()),
        Err(NetworkHttpRefusal::Malformed)
    ));
    let mut request = NetworkRequestBuffer::for_http();
    assert!(matches!(
        request.append(&[0; 1025]),
        Err(NetworkHttpRefusal::TooLarge)
    ));
}

#[test]
fn network_snapshot_routes_pin_fixed_chunks_and_remain_read_only() {
    use crate::network_custody::{self, Reply, Request};
    let mut socket = Socket::new(b"GET /network/custody/offer HTTP/1.1\r\n\r\n");
    assert_eq!(
        poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
        Poll::Ready(Ok(ExchangeOutcome::NetworkSnapshot))
    );
    assert!(core::str::from_utf8(&socket.output[..socket.written])
        .unwrap()
        .contains("503"));
    network_custody::install_provider(|request| match request {
        Request::Offer => Reply::Pending,
        Request::Chunk { snapshot, offset } if snapshot == [0; 32] && offset == 32256 => {
            Reply::Offer(alloc::vec::Vec::from(&b"{\"bytes\":\"ff\"}"[..]))
        }
        _ => Reply::Refused,
    })
    .unwrap();
    let zero = "0".repeat(64);
    for (target, status) in [
        ("/network/custody/offer".into(), "202 Accepted"),
        (
            alloc::format!("/network/custody/chunk?snapshot={zero}&offset=32256"),
            "200 OK",
        ),
        (
            alloc::format!("/network/custody/chunk?snapshot={zero}&offset=0"),
            "409 Conflict",
        ),
    ] {
        let request = alloc::format!("GET {target} HTTP/1.1\r\n\r\n");
        let mut socket = Socket::new(request.as_bytes());
        assert_eq!(
            poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
            Poll::Ready(Ok(ExchangeOutcome::NetworkSnapshot))
        );
        assert!(core::str::from_utf8(&socket.output[..socket.written])
            .unwrap()
            .contains(status));
    }
    for target in [
        alloc::format!("/network/custody/chunk?snapshot={zero}&offset=1"),
        alloc::format!("/network/custody/chunk?snapshot={zero}&offset=32768"),
        alloc::format!("/network/custody/chunk?snapshot={zero}&offset=00"),
        "/network/custody/chunk?snapshot=ff&offset=0".into(),
        alloc::format!("/network/custody/chunk?snapshot={zero}&offset=0&erase=1"),
        "/network/custody/erase".into(),
    ] {
        let request = alloc::format!("GET {target} HTTP/1.1\r\n\r\n");
        let mut socket = Socket::new(request.as_bytes());
        assert!(!matches!(
            poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
            Poll::Ready(Ok(ExchangeOutcome::NetworkSnapshot))
        ));
    }
    let mut socket =
        Socket::new(b"POST /network/custody/offer HTTP/1.1\r\nContent-Length: 0\r\n\r\n");
    assert!(!matches!(
        poll(exchange_with_reading(&mut socket, || 0, None, |_, _| None)),
        Poll::Ready(Ok(ExchangeOutcome::NetworkSnapshot))
    ));
}

#[test]
fn network_ack_is_exact_binary_framing_and_uninstalled_authority_refuses() {
    let body = alloc::vec![0xa5; crate::network_custody::ACK_BYTES];
    let headers = alloc::format!("POST /network/custody/ack HTTP/1.1\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n", body.len());
    let mut request = NetworkRequestBuffer::for_http();
    request.append(headers.as_bytes()).unwrap();
    request.append(&body[..627]).unwrap();
    assert!(matches!(request.take(limits()), Ok(None)));
    request.append(&body[627..]).unwrap();
    assert!(
        matches!(request.take(limits()), Ok(Some(NetworkHttpRequest::NetworkAck(bytes))) if bytes == body)
    );
    assert!(matches!(
        crate::network_custody::dispatch_ack(body),
        crate::network_custody::Reply::Unavailable
    ));
    for length in [0, 627, 629] {
        let wire = alloc::format!("POST /network/custody/ack HTTP/1.1\r\nContent-Type: application/octet-stream\r\nContent-Length: {length}\r\n\r\n");
        let mut request = NetworkRequestBuffer::for_http();
        request.append(wire.as_bytes()).unwrap();
        assert!(matches!(
            request.take(limits()),
            Err(NetworkHttpRefusal::Malformed)
        ));
    }
    for (content_type, extra, expected) in [
        (
            "application/json",
            false,
            NetworkHttpRefusal::UnsupportedContentType,
        ),
        (
            "application/octet-stream",
            true,
            NetworkHttpRefusal::Malformed,
        ),
    ] {
        let wire = alloc::format!("POST /network/custody/ack HTTP/1.1\r\nContent-Type: {content_type}\r\nContent-Length: 628\r\n\r\n");
        let mut request = NetworkRequestBuffer::for_http();
        request.append(wire.as_bytes()).unwrap();
        request.append(&[0; 628]).unwrap();
        if extra {
            request.append(b"x").unwrap();
        }
        assert!(matches!(request.take(limits()), Err(error) if error == expected));
    }
    let mut request = NetworkRequestBuffer::default();
    request.append(headers.as_bytes()).unwrap();
    assert!(matches!(
        request.take(limits()),
        Err(NetworkHttpRefusal::UnsupportedRoute)
    ));
}
