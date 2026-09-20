//! Architecture-neutral, fixed-storage binary WebSocket session.

use embedded_io::{Read, Write};
use embedded_websocket::{
    Error, WebSocketClient, WebSocketOptions, WebSocketReceiveMessageType, WebSocketSendMessageType,
};
use rand_core::RngCore;

const HANDSHAKE_BYTES: usize = 1024;
const FRAME_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebSocketError {
    InvalidDescriptor,
    RequestTooLarge,
    ResponseTooLarge,
    HandshakeWrite,
    HandshakeRead,
    HandshakeAuthentication,
    FrameWrite,
    FrameRead,
    UnexpectedFrame,
    Close,
}

pub(crate) struct BoundedWebSocket<'a, S, R>
where
    S: Read + Write,
    R: RngCore,
{
    stream: &'a mut S,
    websocket: WebSocketClient<R>,
    receive: [u8; FRAME_BYTES],
    received: usize,
    transmit: [u8; FRAME_BYTES],
}

/// Type-erased long-lived binary frame boundary for target protocol adapters.
/// Connection, TLS identity, close, and reconnect authority stay with the
/// enclosing ConduitOS network realization.
pub(crate) trait BinaryWebSocketIo {
    fn send_binary(&mut self, payload: &[u8]) -> Result<(), WebSocketError>;
    fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, WebSocketError>;
}

impl WebSocketError {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidDescriptor => "websocket-descriptor-invalid",
            Self::RequestTooLarge => "websocket-request-too-large",
            Self::ResponseTooLarge => "websocket-response-too-large",
            Self::HandshakeWrite => "websocket-handshake-write-failed",
            Self::HandshakeRead => "websocket-handshake-read-failed",
            Self::HandshakeAuthentication => "websocket-handshake-authentication-failed",
            Self::FrameWrite => "websocket-frame-write-failed",
            Self::FrameRead => "websocket-frame-read-failed",
            Self::UnexpectedFrame => "websocket-frame-unexpected",
            Self::Close => "websocket-close-failed",
        }
    }
}

impl<S, R> BinaryWebSocketIo for BoundedWebSocket<'_, S, R>
where
    S: Read + Write,
    R: RngCore,
{
    fn send_binary(&mut self, payload: &[u8]) -> Result<(), WebSocketError> {
        BoundedWebSocket::send_binary(self, payload)
    }

    fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, WebSocketError> {
        BoundedWebSocket::receive_binary(self, output)
    }
}

impl<'a, S, R> BoundedWebSocket<'a, S, R>
where
    S: Read + Write,
    R: RngCore,
{
    /// Upgrade an already-authenticated stream without taking socket, TLS, or reconnect authority.
    pub(crate) fn connect(
        stream: &'a mut S,
        rng: R,
        host: &str,
        path: &str,
    ) -> Result<Self, WebSocketError> {
        if host.is_empty() || path.is_empty() || !path.starts_with('/') {
            return Err(WebSocketError::InvalidDescriptor);
        }
        let mut websocket = WebSocketClient::new_client(rng);
        let mut handshake = [0; HANDSHAKE_BYTES];
        let options = WebSocketOptions {
            path,
            host,
            origin: "",
            sub_protocols: None,
            additional_headers: None,
        };
        let (request_bytes, key) = websocket
            .client_connect(&options, &mut handshake)
            .map_err(|_| WebSocketError::InvalidDescriptor)?;
        stream
            .write_all(&handshake[..request_bytes])
            .map_err(|_| WebSocketError::HandshakeWrite)?;
        stream.flush().map_err(|_| WebSocketError::HandshakeWrite)?;

        let mut received = 0;
        loop {
            if received == handshake.len() {
                return Err(WebSocketError::HandshakeAuthentication);
            }
            let count = stream
                .read(&mut handshake[received..])
                .map_err(|_| WebSocketError::HandshakeRead)?;
            if count == 0 {
                return Err(WebSocketError::HandshakeRead);
            }
            received += count;
            match websocket.client_accept(&key, &handshake[..received]) {
                Ok((used, _)) => {
                    let trailing = received
                        .checked_sub(used)
                        .ok_or(WebSocketError::HandshakeAuthentication)?;
                    if trailing > FRAME_BYTES {
                        return Err(WebSocketError::ResponseTooLarge);
                    }
                    let mut receive = [0; FRAME_BYTES];
                    receive[..trailing].copy_from_slice(&handshake[used..received]);
                    return Ok(Self {
                        stream,
                        websocket,
                        receive,
                        received: trailing,
                        transmit: [0; FRAME_BYTES],
                    });
                }
                Err(Error::HttpHeaderIncomplete) => {}
                Err(_) => return Err(WebSocketError::HandshakeAuthentication),
            }
        }
    }

    pub(crate) fn send_binary(&mut self, payload: &[u8]) -> Result<(), WebSocketError> {
        if payload.is_empty() || payload.len() + 14 > self.transmit.len() {
            return Err(WebSocketError::RequestTooLarge);
        }
        let bytes = self
            .websocket
            .write(
                WebSocketSendMessageType::Binary,
                true,
                payload,
                &mut self.transmit,
            )
            .map_err(|_| WebSocketError::FrameWrite)?;
        self.stream
            .write_all(&self.transmit[..bytes])
            .map_err(|_| WebSocketError::FrameWrite)?;
        self.stream.flush().map_err(|_| WebSocketError::FrameWrite)
    }

    pub(crate) fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, WebSocketError> {
        if output.is_empty() || output.len() + 14 > self.receive.len() {
            return Err(WebSocketError::ResponseTooLarge);
        }
        let mut written = 0;
        loop {
            match self
                .websocket
                .read(&self.receive[..self.received], &mut output[written..])
            {
                Ok(result) => {
                    if result.message_type != WebSocketReceiveMessageType::Binary
                        || result.len_from > self.received
                    {
                        return Err(WebSocketError::UnexpectedFrame);
                    }
                    written = written
                        .checked_add(result.len_to)
                        .ok_or(WebSocketError::ResponseTooLarge)?;
                    self.receive.copy_within(result.len_from..self.received, 0);
                    self.received -= result.len_from;
                    if result.end_of_message {
                        return Ok(written);
                    }
                    if written == output.len() {
                        return Err(WebSocketError::ResponseTooLarge);
                    }
                    if self.received != 0 {
                        continue;
                    }
                }
                Err(Error::ReadFrameIncomplete) => {}
                Err(_) => return Err(WebSocketError::UnexpectedFrame),
            }
            if self.received == self.receive.len() {
                return Err(WebSocketError::ResponseTooLarge);
            }
            let count = self
                .stream
                .read(&mut self.receive[self.received..])
                .map_err(|_| WebSocketError::FrameRead)?;
            if count == 0 {
                return Err(WebSocketError::FrameRead);
            }
            self.received += count;
        }
    }

    pub(crate) fn close(mut self) -> Result<(), WebSocketError> {
        let bytes = self
            .websocket
            .close(
                embedded_websocket::WebSocketCloseStatusCode::NormalClosure,
                None,
                &mut self.transmit,
            )
            .map_err(|_| WebSocketError::Close)?;
        self.stream
            .write_all(&self.transmit[..bytes])
            .map_err(|_| WebSocketError::Close)?;
        self.stream.flush().map_err(|_| WebSocketError::Close)
    }
}

#[cfg(test)]
fn exchange<S, R>(
    stream: &mut S,
    rng: R,
    host: &str,
    path: &str,
    request: &[u8],
    response: &mut [u8],
    expected_response_bytes: usize,
) -> Result<(), WebSocketError>
where
    S: Read + Write,
    R: RngCore,
{
    if request.is_empty() || request.len() + 14 > FRAME_BYTES {
        return Err(WebSocketError::RequestTooLarge);
    }
    if expected_response_bytes == 0 || expected_response_bytes > response.len() {
        return Err(WebSocketError::ResponseTooLarge);
    }
    let mut websocket = BoundedWebSocket::connect(stream, rng, host, path)?;
    websocket.send_binary(request)?;
    let received = websocket.receive_binary(response)?;
    if received != expected_response_bytes {
        return Err(WebSocketError::UnexpectedFrame);
    }
    websocket.close()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;
    use rand_core::SeedableRng;

    struct Untouched;

    struct Scripted<'a> {
        input: &'a [u8],
    }

    struct CoalescedUpgrade {
        request: [u8; HANDSHAKE_BYTES],
        request_len: usize,
        response: [u8; HANDSHAKE_BYTES],
        response_len: usize,
        response_offset: usize,
    }

    impl CoalescedUpgrade {
        const fn new() -> Self {
            Self {
                request: [0; HANDSHAKE_BYTES],
                request_len: 0,
                response: [0; HANDSHAKE_BYTES],
                response_len: 0,
                response_offset: 0,
            }
        }

        fn prepare_response(&mut self) {
            let prefix = b"Sec-WebSocket-Key: ";
            let key_start = self.request[..self.request_len]
                .windows(prefix.len())
                .position(|window| window == prefix)
                .expect("client request has a WebSocket key")
                + prefix.len();
            let key_end = self.request[key_start..self.request_len]
                .windows(2)
                .position(|window| window == b"\r\n")
                .expect("WebSocket key header is terminated")
                + key_start;
            let key = core::str::from_utf8(&self.request[key_start..key_end])
                .expect("WebSocket key is ASCII");
            let key = embedded_websocket::WebSocketKey::from(key);
            let mut server = embedded_websocket::WebSocketServer::new_server();
            let mut used = server
                .server_accept(&key, None, &mut self.response)
                .expect("server accepts client key");
            used += server
                .write(
                    WebSocketSendMessageType::Binary,
                    false,
                    b"one-",
                    &mut self.response[used..],
                )
                .expect("first fragment fits");
            used += server
                .write(
                    WebSocketSendMessageType::Binary,
                    true,
                    b"two",
                    &mut self.response[used..],
                )
                .expect("final fragment fits");
            used += server
                .write(
                    WebSocketSendMessageType::Binary,
                    true,
                    b"next",
                    &mut self.response[used..],
                )
                .expect("second message fits");
            self.response_len = used;
        }
    }

    impl embedded_io::ErrorType for Untouched {
        type Error = Infallible;
    }

    impl Read for Untouched {
        fn read(&mut self, _: &mut [u8]) -> Result<usize, Self::Error> {
            panic!("invalid admission reached I/O")
        }
    }

    impl Write for Untouched {
        fn write(&mut self, _: &[u8]) -> Result<usize, Self::Error> {
            panic!("invalid admission reached I/O")
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            panic!("invalid admission reached I/O")
        }
    }

    impl embedded_io::ErrorType for Scripted<'_> {
        type Error = Infallible;
    }

    impl Read for Scripted<'_> {
        fn read(&mut self, output: &mut [u8]) -> Result<usize, Self::Error> {
            let count = output.len().min(self.input.len());
            output[..count].copy_from_slice(&self.input[..count]);
            self.input = &self.input[count..];
            Ok(count)
        }
    }

    impl Write for Scripted<'_> {
        fn write(&mut self, input: &[u8]) -> Result<usize, Self::Error> {
            Ok(input.len())
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl embedded_io::ErrorType for CoalescedUpgrade {
        type Error = Infallible;
    }

    impl Read for CoalescedUpgrade {
        fn read(&mut self, output: &mut [u8]) -> Result<usize, Self::Error> {
            if self.response_len == 0 {
                self.prepare_response();
            }
            let remaining = &self.response[self.response_offset..self.response_len];
            let count = output.len().min(remaining.len());
            output[..count].copy_from_slice(&remaining[..count]);
            self.response_offset += count;
            Ok(count)
        }
    }

    impl Write for CoalescedUpgrade {
        fn write(&mut self, input: &[u8]) -> Result<usize, Self::Error> {
            let available = self.request.len() - self.request_len;
            let count = input.len().min(available);
            self.request[self.request_len..self.request_len + count]
                .copy_from_slice(&input[..count]);
            self.request_len += count;
            Ok(count)
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[test]
    fn descriptors_and_storage_refuse_before_stream_use() {
        let mut response = [0; 1];
        let make_rng = || rand_chacha::ChaCha20Rng::from_seed([7; 32]);
        assert_eq!(
            exchange(
                &mut Untouched,
                make_rng(),
                "",
                "/conduit",
                b"x",
                &mut response,
                1,
            ),
            Err(WebSocketError::InvalidDescriptor)
        );
        assert_eq!(
            exchange(
                &mut Untouched,
                make_rng(),
                "relay",
                "relative",
                b"x",
                &mut response,
                1,
            ),
            Err(WebSocketError::InvalidDescriptor)
        );
        assert_eq!(
            exchange(
                &mut Untouched,
                make_rng(),
                "relay",
                "/conduit",
                b"",
                &mut response,
                1,
            ),
            Err(WebSocketError::RequestTooLarge)
        );
        assert_eq!(
            exchange(
                &mut Untouched,
                make_rng(),
                "relay",
                "/conduit",
                b"x",
                &mut response,
                0,
            ),
            Err(WebSocketError::ResponseTooLarge)
        );
    }

    #[test]
    fn rejected_and_lost_upgrades_remain_distinct() {
        let mut response = [0; 1];
        let rng = rand_chacha::ChaCha20Rng::from_seed([8; 32]);
        let mut rejected = Scripted {
            input: b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n",
        };
        assert_eq!(
            exchange(
                &mut rejected,
                rng,
                "relay",
                "/conduit",
                b"x",
                &mut response,
                1,
            ),
            Err(WebSocketError::HandshakeAuthentication)
        );

        let rng = rand_chacha::ChaCha20Rng::from_seed([9; 32]);
        let mut lost = Scripted { input: b"" };
        assert_eq!(
            exchange(&mut lost, rng, "relay", "/conduit", b"x", &mut response, 1,),
            Err(WebSocketError::HandshakeRead)
        );
    }

    #[test]
    fn coalesced_upgrade_fragmented_message_and_next_message_reuse_fixed_storage() {
        let mut stream = CoalescedUpgrade::new();
        let rng = rand_chacha::ChaCha20Rng::from_seed([10; 32]);
        let mut websocket = BoundedWebSocket::connect(&mut stream, rng, "relay", "/conduit")
            .expect("coalesced upgrade succeeds");
        assert_eq!(
            websocket.received, 17,
            "all coalesced frames remain buffered"
        );
        assert_eq!(
            &websocket.receive[..websocket.received],
            b"\x02\x04one-\x80\x03two\x82\x04next"
        );
        let mut first = [0; 7];
        assert_eq!(websocket.receive_binary(&mut first), Ok(7));
        assert_eq!(&first, b"one-two");
        let mut second = [0; 4];
        assert_eq!(websocket.receive_binary(&mut second), Ok(4));
        assert_eq!(&second, b"next");
        websocket.close().expect("bounded close fits");
    }
}
