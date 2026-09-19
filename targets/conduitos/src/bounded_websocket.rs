//! Architecture-neutral, fixed-storage binary WebSocket exchange.

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
                Ok((used, _)) if used == received => break,
                Ok(_) => return Err(WebSocketError::HandshakeAuthentication),
                Err(Error::HttpHeaderIncomplete) => {}
                Err(_) => return Err(WebSocketError::HandshakeAuthentication),
            }
        }
        Ok(Self {
            stream,
            websocket,
            receive: [0; FRAME_BYTES],
            received: 0,
            transmit: [0; FRAME_BYTES],
        })
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

pub(crate) fn exchange<S, R>(
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
}
