use super::*;
#[test]
fn strict_read_only_grammar_pins_one_bounded_complete_snapshot() {
    let digest = "ab".repeat(32);
    let nonce = alloc::format!("GET /face/export/snapshot?nonce={digest} HTTP/1.1");
    assert_eq!(
        parse(&nonce),
        Ok(Some(Request::Snapshot { nonce: [0xab; 32] }))
    );
    let chunk = alloc::format!(
        "GET /face/export/chunk?digest={digest}&revision=7&bytes=8192&offset=7680 HTTP/1.0"
    );
    assert_eq!(
        parse(&chunk),
        Ok(Some(Request::Chunk {
            digest: [0xab; 32],
            revision_le: 7u64.to_le_bytes(),
            bytes: 8192,
            offset: 7680
        }))
    );
    for suffix in [
        "&revision=7&bytes=8193&offset=0",
        "&revision=7&bytes=8192&offset=1",
        "&revision=7&bytes=512&offset=512",
        "&revision=07&bytes=512&offset=0",
        "&revision=7&bytes=512&offset=0&erase=1",
    ] {
        assert!(parse(&alloc::format!(
            "GET /face/export/chunk?digest={digest}{suffix} HTTP/1.1"
        ))
        .is_err());
    }
    assert!(parse(&alloc::format!(
        "GET /face/export/snapshot?nonce={} HTTP/1.1",
        "00".repeat(32)
    ))
    .is_err());
    assert_eq!(parse("POST /face/export/snapshot HTTP/1.1"), Ok(None));
}

#[test]
fn actual_http_dispatch_refuses_before_root_installs_a_provider() {
    use core::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };
    struct Socket {
        input: &'static [u8],
        output: alloc::vec::Vec<u8>,
    }
    impl embedded_io_async::ErrorType for Socket {
        type Error = core::convert::Infallible;
    }
    impl embedded_io_async::Read for Socket {
        async fn read(&mut self, output: &mut [u8]) -> Result<usize, Self::Error> {
            let length = output.len().min(self.input.len());
            output[..length].copy_from_slice(&self.input[..length]);
            self.input = &self.input[length..];
            Ok(length)
        }
    }
    impl embedded_io_async::Write for Socket {
        async fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
            self.output.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        async fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    let mut socket = Socket { input: b"GET /face/export/snapshot?nonce=abababababababababababababababababababababababababababababababab HTTP/1.1\r\n\r\n", output: alloc::vec::Vec::new() };
    let result = {
        let mut future = pin!(crate::exchange::exchange_with_reading(
            &mut socket,
            || 0,
            None,
            |_, _| None
        ));
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
    };
    assert_eq!(
        result,
        Poll::Ready(Ok(crate::exchange::ExchangeOutcome::FaceExport))
    );
    assert!(core::str::from_utf8(&socket.output)
        .unwrap()
        .starts_with("HTTP/1.1 503 Service Unavailable"));
}
