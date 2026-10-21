//! One connection and finite transport transactions; no network effect is executed.
use crate::{
    MAX_BLOB, Refusal,
    framer::{Framer, Step, buffer},
    memory::TRANSFER_BYTES,
    wire::{InputKind, OutputKind, Request, Response},
};
use alloc::vec::Vec;
#[derive(Default)]
pub struct State {
    framer: Option<Framer>,
    started: bool,
    incoming: Option<(InputKind, usize, Vec<u8>)>,
    outgoing: Vec<u8>,
    read: usize,
}
/// Raw initialization payload: seed32, hostlenLEu16, pathlenLEu16, hostUTF8,pathUTF8.
/// Root supplies its already-approved host/path and cryptographic WebSocket seed.
pub fn encode_initialize(seed: [u8; 32], host: &str, path: &str) -> Result<Vec<u8>, Refusal> {
    let host_len = u16::try_from(host.len()).map_err(|_| Refusal::Bounds)?;
    let path_len = u16::try_from(path.len()).map_err(|_| Refusal::Bounds)?;
    let total = 36usize
        .checked_add(host.len())
        .and_then(|n| n.checked_add(path.len()))
        .filter(|n| *n <= MAX_BLOB)
        .ok_or(Refusal::Bounds)?;
    let mut out = buffer(total)?;
    out[..32].copy_from_slice(&seed);
    out[32..34].copy_from_slice(&host_len.to_le_bytes());
    out[34..36].copy_from_slice(&path_len.to_le_bytes());
    out[36..36 + host.len()].copy_from_slice(host.as_bytes());
    out[36 + host.len()..].copy_from_slice(path.as_bytes());
    Ok(out)
}
impl State {
    pub fn execute(&mut self, request: Request, transfer: &mut [u8]) -> Result<Response, Refusal> {
        if transfer.len() != TRANSFER_BYTES {
            return Err(Refusal::Bounds);
        }
        if self.incoming.is_some() && !matches!(request, Request::Write { .. } | Request::Commit) {
            return Err(Refusal::Order);
        }
        if !self.outgoing.is_empty() && !matches!(request, Request::Read { .. }) {
            return Err(Refusal::Order);
        }
        match request {
            Request::Begin { kind, bytes } => {
                if self.incoming.is_some() || bytes == 0 || bytes as usize > MAX_BLOB {
                    return Err(Refusal::Order);
                }
                if (kind == InputKind::Initialize) != (!self.started) {
                    return Err(Refusal::Order);
                }
                let mut data = Vec::new();
                data.try_reserve_exact(bytes as usize)
                    .map_err(|_| Refusal::Capacity)?;
                self.incoming = Some((kind, bytes as usize, data));
                Ok(Response::Ready)
            }
            Request::Write { offset, bytes } => {
                let (_, total, input) = self.incoming.as_mut().ok_or(Refusal::Order)?;
                let end = (offset as usize)
                    .checked_add(bytes as usize)
                    .filter(|n| *n <= *total)
                    .ok_or(Refusal::Bounds)?;
                if offset as usize != input.len() || bytes == 0 || bytes as usize > TRANSFER_BYTES {
                    return Err(Refusal::Order);
                }
                input.extend_from_slice(&transfer[..bytes as usize]);
                if input.len() != end {
                    return Err(Refusal::Order);
                }
                Ok(Response::Ready)
            }
            Request::Commit => {
                let (kind, total, input) = self.incoming.take().ok_or(Refusal::Order)?;
                if input.len() != total {
                    return Err(Refusal::Order);
                }
                self.commit(kind, &input)
            }
            Request::StartReceive { maximum } => {
                let step = self
                    .framer
                    .as_mut()
                    .ok_or(Refusal::Order)?
                    .start_receive(maximum as usize)?;
                self.step(step)
            }
            Request::StepReceive => {
                let step = self.framer.as_mut().ok_or(Refusal::Order)?.step()?;
                self.step(step)
            }
            Request::Close => {
                let bytes = self.framer.as_mut().ok_or(Refusal::Order)?.close()?;
                self.publish(OutputKind::WireFrame, bytes)
            }
            Request::Read { offset, maximum } => {
                if offset as usize != self.read
                    || maximum == 0
                    || maximum as usize > TRANSFER_BYTES
                    || self.read >= self.outgoing.len()
                {
                    return Err(Refusal::Order);
                }
                let bytes = (maximum as usize).min(self.outgoing.len() - self.read);
                transfer.fill(0);
                transfer[..bytes].copy_from_slice(&self.outgoing[self.read..self.read + bytes]);
                self.read += bytes;
                let total = self.outgoing.len();
                if self.read == total {
                    self.outgoing.clear();
                    self.read = 0;
                }
                Ok(Response::Chunk {
                    offset,
                    bytes: bytes as u32,
                    total: total as u32,
                })
            }
        }
    }
    fn commit(&mut self, kind: InputKind, input: &[u8]) -> Result<Response, Refusal> {
        match kind {
            InputKind::Initialize => {
                if self.started {
                    return Err(Refusal::Order);
                }
                self.started = true;
                if input.len() < 36 {
                    return Err(Refusal::Malformed);
                }
                let seed = input[..32].try_into().map_err(|_| Refusal::Malformed)?;
                let host_len =
                    u16::from_le_bytes(input[32..34].try_into().map_err(|_| Refusal::Malformed)?)
                        as usize;
                let path_len =
                    u16::from_le_bytes(input[34..36].try_into().map_err(|_| Refusal::Malformed)?)
                        as usize;
                if 36 + host_len + path_len != input.len() {
                    return Err(Refusal::Malformed);
                }
                let host = core::str::from_utf8(&input[36..36 + host_len])
                    .map_err(|_| Refusal::Malformed)?;
                let path = core::str::from_utf8(&input[36 + host_len..])
                    .map_err(|_| Refusal::Malformed)?;
                let (framer, bytes) = Framer::initialize(seed, host, path)?;
                self.framer = Some(framer);
                self.publish(OutputKind::Upgrade, bytes)
            }
            InputKind::Handshake => {
                match self
                    .framer
                    .as_mut()
                    .ok_or(Refusal::Order)?
                    .handshake(input)?
                {
                    None => Ok(Response::Ready),
                    Some(maximum) => Ok(Response::NeedInput {
                        maximum: maximum as u32,
                    }),
                }
            }
            InputKind::SendBinary => {
                let bytes = self.framer.as_mut().ok_or(Refusal::Order)?.send(input)?;
                self.publish(OutputKind::WireFrame, bytes)
            }
            InputKind::Receive => {
                self.framer.as_mut().ok_or(Refusal::Order)?.ingest(input)?;
                Ok(Response::Continue)
            }
        }
    }
    fn publish(&mut self, kind: OutputKind, bytes: Vec<u8>) -> Result<Response, Refusal> {
        if bytes.len() > MAX_BLOB {
            return Err(Refusal::Bounds);
        }
        self.outgoing = bytes;
        self.read = 0;
        Ok(Response::Blob {
            kind,
            bytes: self.outgoing.len() as u32,
        })
    }
    fn step(&mut self, step: Step) -> Result<Response, Refusal> {
        match step {
            Step::NeedInput(maximum) => Ok(Response::NeedInput {
                maximum: maximum as u32,
            }),
            Step::Continue => Ok(Response::Continue),
            Step::Message(bytes) => self.publish(OutputKind::BinaryMessage, bytes),
        }
    }
}
