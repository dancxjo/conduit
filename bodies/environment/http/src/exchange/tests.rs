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
