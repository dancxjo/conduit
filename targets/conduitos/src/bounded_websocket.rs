//! Architecture-neutral, fixed-storage binary WebSocket session.

use embedded_io::{Read, Write};
use embedded_websocket::{
    Error, WebSocketClient, WebSocketOptions, WebSocketReceiveMessageType, WebSocketSendMessageType,
};
use rand_core::RngCore;

const HANDSHAKE_BYTES: usize = 1024;
const FRAME_BYTES: usize = crate::native_network_bounds::WEBSOCKET_FRAME_BYTES;
pub(crate) const MAXIMUM_BINARY_MESSAGE_BYTES: usize =
    crate::native_network_bounds::MAXIMUM_BINARY_MESSAGE_BYTES;

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
#[path = "bounded_websocket_tests.rs"]
mod tests;
