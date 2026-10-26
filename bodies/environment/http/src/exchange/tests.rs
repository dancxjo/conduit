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
