//! Root owns authenticated IO; the isolated Network lease owns the WebSocket codec.
mod transport;
use crate::bounded_websocket::BinaryWebSocketIo;
use conduitos_network_core::{
    memory::TRANSFER_BYTES,
    state::encode_initialize,
    wire::{InputKind, OutputKind, Request, Response},
    WebSocketError as E, FRAME_BYTES, HANDSHAKE_BYTES, MAXIMUM_BINARY_MESSAGE_BYTES,
};
use core::marker::PhantomData;
use embedded_io::{Read, Write};
use rand_core::RngCore;
use transport::Service;

// A finite engineering ceiling, including fragmented frames and empty fragments.
const PARSE_STEPS: usize = FRAME_BYTES;
const RECEIVE_WIRE_BYTES: usize = FRAME_BYTES * 16;
pub(crate) struct BoundedWebSocket<'a, S: Read + Write, R: RngCore> {
    stream: &'a mut S,
    service: Service,
    buffered: usize,
    rng: PhantomData<R>,
}
impl<'a, S: Read + Write, R: RngCore> BoundedWebSocket<'a, S, R> {
    pub(crate) fn connect(
        stream: &'a mut S,
        mut rng: R,
        host: &str,
        path: &str,
    ) -> Result<Self, E> {
        if host.is_empty() || !path.starts_with('/') {
            return Err(E::InvalidDescriptor);
        }
        let mut seed = [0; 32];
        rng.try_fill_bytes(&mut seed)
            .map_err(|_| E::InvalidDescriptor)?;
        let mut initialization =
            encode_initialize(seed, host, path).map_err(|_| E::InvalidDescriptor)?;
        seed.fill(0);
        let mut session = Self {
            stream,
            service: Service::install()?,
            buffered: 0,
            rng: PhantomData,
        };
        let result = session.upgrade(&initialization);
        initialization.fill(0);
        if let Err(error) = result {
            session.service.abort(error);
            return Err(error);
        }
        Ok(session)
    }
    fn upgrade(&mut self, initialization: &[u8]) -> Result<(), E> {
        let response = self.service.input(InputKind::Initialize, initialization)?;
        let Response::Blob {
            kind: OutputKind::Upgrade,
            bytes,
            buffered: 0,
        } = response
        else {
            return Err(E::UnexpectedFrame);
        };
        if bytes == 0 || bytes as usize > HANDSHAKE_BYTES {
            return Err(E::UnexpectedFrame);
        }
        let mut buffer = [0; HANDSHAKE_BYTES];
        let count = self
            .service
            .blob(response, OutputKind::Upgrade, &mut buffer)?;
        self.stream
            .write_all(&buffer[..count])
            .map_err(|_| E::HandshakeWrite)?;
        self.stream.flush().map_err(|_| E::HandshakeWrite)?;
        let mut received = 0;
        let mut allowance = HANDSHAKE_BYTES;
        loop {
            if allowance == 0 || allowance > HANDSHAKE_BYTES - received {
                return Err(E::HandshakeAuthentication);
            }
            let count = self
                .stream
                .read(&mut buffer[..allowance])
                .map_err(|_| E::HandshakeRead)?;
            if count == 0 || count > allowance {
                return Err(E::HandshakeRead);
            }
            received += count;
            match self.service.input(InputKind::Handshake, &buffer[..count])? {
                Response::Accepted { trailing } if trailing as usize <= received => {
                    self.buffered = trailing as usize;
                    return Ok(());
                }
                Response::NeedInput {
                    maximum,
                    consumed: 0,
                    written: 0,
                } => allowance = maximum as usize,
                _ => return Err(E::UnexpectedFrame),
            }
        }
    }
    fn write_blob(&mut self, response: Response, maximum: usize, error: E) -> Result<(), E> {
        let Response::Blob {
            kind: OutputKind::WireFrame,
            bytes,
            buffered,
        } = response
        else {
            return Err(E::UnexpectedFrame);
        };
        if bytes == 0 || bytes as usize > maximum || buffered as usize != self.buffered {
            return Err(E::UnexpectedFrame);
        }
        let mut wire = [0; FRAME_BYTES];
        let count = self
            .service
            .blob(response, OutputKind::WireFrame, &mut wire)?;
        self.stream.write_all(&wire[..count]).map_err(|_| error)?;
        self.stream.flush().map_err(|_| error)
    }
    pub(crate) fn send_binary(&mut self, payload: &[u8]) -> Result<(), E> {
        let result = (|| {
            self.service.live()?;
            if payload.is_empty() || payload.len() > MAXIMUM_BINARY_MESSAGE_BYTES {
                return Err(E::RequestTooLarge);
            }
            let response = self.service.input(InputKind::SendBinary, payload)?;
            self.write_blob(response, payload.len() + 14, E::FrameWrite)
        })();
        if let Err(error) = result {
            self.service.abort(error);
        }
        result
    }
    pub(crate) fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, E> {
        let result = self.receive(output);
        if let Err(error) = result {
            self.service.abort(error);
        }
        result
    }
    fn progress(
        &mut self,
        consumed: u32,
        written: u32,
        produced: &mut usize,
        maximum: usize,
    ) -> Result<(), E> {
        let (consumed, written) = (consumed as usize, written as usize);
        if consumed > self.buffered || written > consumed || written > maximum - *produced {
            return Err(E::UnexpectedFrame);
        }
        self.buffered -= consumed;
        *produced += written;
        Ok(())
    }
    fn receive(&mut self, output: &mut [u8]) -> Result<usize, E> {
        self.service.live()?;
        if output.is_empty() || output.len() > MAXIMUM_BINARY_MESSAGE_BYTES {
            return Err(E::ResponseTooLarge);
        }
        let mut response = self.service.request(Request::StartReceive {
            maximum: output.len() as u32,
        })?;
        let mut produced = 0;
        let mut read = 0;
        let mut input = [0; TRANSFER_BYTES];
        for _ in 0..PARSE_STEPS {
            match response {
                Response::Blob {
                    kind: OutputKind::BinaryMessage,
                    bytes,
                    buffered,
                } => {
                    let (bytes, buffered) = (bytes as usize, buffered as usize);
                    if bytes < produced
                        || bytes > output.len()
                        || buffered > self.buffered
                        || bytes - produced > self.buffered - buffered
                    {
                        return Err(E::UnexpectedFrame);
                    }
                    self.buffered = buffered;
                    return self
                        .service
                        .blob(response, OutputKind::BinaryMessage, output);
                }
                Response::Continue { consumed, written } => {
                    if consumed == 0 && written == 0 {
                        return Err(E::UnexpectedFrame);
                    }
                    self.progress(consumed, written, &mut produced, output.len())?;
                }
                Response::NeedInput {
                    maximum,
                    consumed,
                    written,
                } => {
                    self.progress(consumed, written, &mut produced, output.len())?;
                    let maximum = maximum as usize;
                    if maximum == 0 || maximum > FRAME_BYTES - self.buffered {
                        return Err(E::UnexpectedFrame);
                    }
                    let allowance = maximum.min(input.len()).min(RECEIVE_WIRE_BYTES - read);
                    if allowance == 0 {
                        return Err(E::ResponseTooLarge);
                    }
                    let count = self
                        .stream
                        .read(&mut input[..allowance])
                        .map_err(|_| E::FrameRead)?;
                    if count == 0 || count > allowance {
                        return Err(E::FrameRead);
                    }
                    read += count;
                    self.buffered += count;
                    if self.service.input(InputKind::Receive, &input[..count])?
                        != (Response::Continue {
                            consumed: 0,
                            written: 0,
                        })
                    {
                        return Err(E::UnexpectedFrame);
                    }
                }
                _ => return Err(E::UnexpectedFrame),
            }
            response = self.service.request(Request::StepReceive)?;
        }
        Err(E::ResponseTooLarge)
    }
    pub(crate) fn close(mut self) -> Result<(), E> {
        let result = (|| {
            self.service.live()?;
            let response = self.service.request(Request::Close)?;
            self.write_blob(response, 14 + 125, E::Close)
        })();
        if let Err(error) = result {
            self.service.abort(error);
        }
        result
    }
}
impl<S: Read + Write, R: RngCore> BinaryWebSocketIo for BoundedWebSocket<'_, S, R> {
    fn send_binary(&mut self, payload: &[u8]) -> Result<(), E> {
        Self::send_binary(self, payload)
    }
    fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, E> {
        Self::receive_binary(self, output)
    }
}
