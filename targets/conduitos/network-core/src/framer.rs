//! The existing embedded_websocket calls and byte-buffer transitions, split at
//! stream IO. One step performs one codec read; Root pumps IO between steps.
use crate::{
    FRAME_BYTES, HANDSHAKE_BYTES, MAXIMUM_BINARY_MESSAGE_BYTES, Refusal, WebSocketError as E,
};
use alloc::vec::Vec;
use embedded_websocket::{
    Error, WebSocketClient, WebSocketCloseStatusCode, WebSocketKey, WebSocketOptions,
    WebSocketReceiveMessageType, WebSocketSendMessageType,
};
pub enum Step {
    NeedInput(usize),
    Continue,
    Message(Vec<u8>),
}
pub struct Framer {
    websocket: WebSocketClient<crate::rng::ErasedChaCha>,
    key: Option<WebSocketKey>,
    handshake: Vec<u8>,
    handshake_received: usize,
    receive: Vec<u8>,
    received: usize,
    transmit: Vec<u8>,
    message: Option<Vec<u8>>,
    written: usize,
    closed: bool,
    awaiting: Option<usize>,
}
pub(crate) fn buffer(bytes: usize) -> Result<Vec<u8>, Refusal> {
    let mut value = Vec::new();
    value
        .try_reserve_exact(bytes)
        .map_err(|_| Refusal::Capacity)?;
    value.resize(bytes, 0);
    Ok(value)
}
impl Framer {
    /// Seed must be the enclosing Root realization's independent cryptographic
    /// WebSocket seed. It conveys neither endpoint nor TLS authentication authority.
    pub fn initialize(seed: [u8; 32], host: &str, path: &str) -> Result<(Self, Vec<u8>), Refusal> {
        if host.is_empty() || path.is_empty() || !path.starts_with('/') {
            return Err(Refusal::WebSocket(E::InvalidDescriptor));
        }
        let mut websocket = WebSocketClient::new_client(crate::rng::ErasedChaCha::new(seed));
        let mut handshake = buffer(HANDSHAKE_BYTES)?;
        let options = WebSocketOptions {
            path,
            host,
            origin: "",
            sub_protocols: None,
            additional_headers: None,
        };
        let (length, key) = websocket
            .client_connect(&options, &mut handshake)
            .map_err(|_| Refusal::WebSocket(E::InvalidDescriptor))?;
        let mut output = buffer(length)?;
        output.copy_from_slice(&handshake[..length]);
        handshake.fill(0);
        Ok((
            Self {
                websocket,
                key: Some(key),
                handshake,
                handshake_received: 0,
                receive: buffer(FRAME_BYTES)?,
                received: 0,
                transmit: buffer(FRAME_BYTES)?,
                message: None,
                written: 0,
                closed: false,
                awaiting: None,
            },
            output,
        ))
    }
    pub fn handshake(&mut self, bytes: &[u8]) -> Result<Option<usize>, Refusal> {
        let key = self.key.as_ref().ok_or(Refusal::Order)?;
        if bytes.is_empty() {
            return Err(Refusal::WebSocket(E::HandshakeRead));
        }
        let end = self
            .handshake_received
            .checked_add(bytes.len())
            .ok_or(Refusal::Bounds)?;
        if end > HANDSHAKE_BYTES {
            return Err(Refusal::WebSocket(E::HandshakeAuthentication));
        }
        self.handshake[self.handshake_received..end].copy_from_slice(bytes);
        self.handshake_received = end;
        match self.websocket.client_accept(key, &self.handshake[..end]) {
            Ok((used, _)) => {
                let trailing = end
                    .checked_sub(used)
                    .ok_or(Refusal::WebSocket(E::HandshakeAuthentication))?;
                if trailing > FRAME_BYTES {
                    return Err(Refusal::WebSocket(E::ResponseTooLarge));
                }
                self.receive[..trailing].copy_from_slice(&self.handshake[used..end]);
                self.received = trailing;
                self.handshake.fill(0);
                self.key = None;
                Ok(None)
            }
            Err(Error::HttpHeaderIncomplete) => {
                if end == HANDSHAKE_BYTES {
                    return Err(Refusal::WebSocket(E::HandshakeAuthentication));
                }
                Ok(Some(HANDSHAKE_BYTES - end))
            }
            Err(_) => Err(Refusal::WebSocket(E::HandshakeAuthentication)),
        }
    }
    fn open(&self) -> Result<(), Refusal> {
        if self.key.is_some() || self.closed {
            Err(Refusal::Order)
        } else {
            Ok(())
        }
    }
    pub fn send(&mut self, payload: &[u8]) -> Result<Vec<u8>, Refusal> {
        self.open()?;
        if self.message.is_some() {
            return Err(Refusal::Order);
        }
        if payload.is_empty() || payload.len() > MAXIMUM_BINARY_MESSAGE_BYTES {
            return Err(Refusal::WebSocket(E::RequestTooLarge));
        }
        let length = self
            .websocket
            .write(
                WebSocketSendMessageType::Binary,
                true,
                payload,
                &mut self.transmit,
            )
            .map_err(|_| Refusal::WebSocket(E::FrameWrite))?;
        let mut output = buffer(length)?;
        output.copy_from_slice(&self.transmit[..length]);
        Ok(output)
    }
    pub fn start_receive(&mut self, maximum: usize) -> Result<Step, Refusal> {
        self.open()?;
        if self.message.is_some() {
            return Err(Refusal::Order);
        }
        if maximum == 0 || maximum > MAXIMUM_BINARY_MESSAGE_BYTES {
            return Err(Refusal::WebSocket(E::ResponseTooLarge));
        }
        self.message = Some(buffer(maximum)?);
        self.written = 0;
        self.step()
    }
    pub fn ingest(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        self.open()?;
        if self.message.is_none() {
            return Err(Refusal::Order);
        }
        if bytes.is_empty() {
            return Err(Refusal::WebSocket(E::FrameRead));
        }
        let maximum = self.awaiting.ok_or(Refusal::Order)?;
        if bytes.len() > maximum {
            return Err(Refusal::Bounds);
        }
        let end = self
            .received
            .checked_add(bytes.len())
            .ok_or(Refusal::Bounds)?;
        if end > FRAME_BYTES {
            return Err(Refusal::WebSocket(E::ResponseTooLarge));
        }
        self.receive[self.received..end].copy_from_slice(bytes);
        self.received = end;
        self.awaiting = None;
        Ok(())
    }
    pub fn step(&mut self) -> Result<Step, Refusal> {
        let result = self.read_step();
        if result.is_err() {
            self.message = None;
            self.awaiting = None;
        }
        result
    }
    fn read_step(&mut self) -> Result<Step, Refusal> {
        self.open()?;
        if self.awaiting.is_some() {
            return Err(Refusal::Order);
        }
        let output = self.message.as_mut().ok_or(Refusal::Order)?;
        match self
            .websocket
            .read(&self.receive[..self.received], &mut output[self.written..])
        {
            Ok(result) => {
                if result.message_type != WebSocketReceiveMessageType::Binary
                    || result.len_from > self.received
                {
                    return Err(Refusal::WebSocket(E::UnexpectedFrame));
                }
                self.written = self
                    .written
                    .checked_add(result.len_to)
                    .filter(|n| *n <= output.len())
                    .ok_or(Refusal::WebSocket(E::ResponseTooLarge))?;
                self.receive.copy_within(result.len_from..self.received, 0);
                self.received -= result.len_from;
                if result.end_of_message {
                    let mut output = self.message.take().ok_or(Refusal::Order)?;
                    output.truncate(self.written);
                    return Ok(Step::Message(output));
                }
                if self.written == output.len() {
                    return Err(Refusal::WebSocket(E::ResponseTooLarge));
                }
                if self.received != 0 {
                    return Ok(Step::Continue);
                }
            }
            Err(Error::ReadFrameIncomplete) => {}
            Err(_) => return Err(Refusal::WebSocket(E::UnexpectedFrame)),
        }
        if self.received == FRAME_BYTES {
            return Err(Refusal::WebSocket(E::ResponseTooLarge));
        }
        self.awaiting = Some(FRAME_BYTES - self.received);
        Ok(Step::NeedInput(FRAME_BYTES - self.received))
    }
    pub fn close(&mut self) -> Result<Vec<u8>, Refusal> {
        self.open()?;
        let length = self
            .websocket
            .close(
                WebSocketCloseStatusCode::NormalClosure,
                None,
                &mut self.transmit,
            )
            .map_err(|_| Refusal::WebSocket(E::Close))?;
        self.closed = true;
        let mut output = buffer(length)?;
        output.copy_from_slice(&self.transmit[..length]);
        Ok(output)
    }
}
