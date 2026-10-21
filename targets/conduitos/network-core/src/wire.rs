//! Versioned little-endian descriptors; bulk bytes are raw, never serialized Vecs.
use crate::{
    MAX_BLOB, MAXIMUM_BINARY_MESSAGE_BYTES, Refusal, WebSocketError, memory::TRANSFER_BYTES,
};
pub const BYTES: usize = 48;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Envelope {
    pub instance: u64,
    pub sequence: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum InputKind {
    Initialize = 1,
    Handshake = 2,
    SendBinary = 3,
    Receive = 4,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum OutputKind {
    Upgrade = 1,
    WireFrame = 2,
    BinaryMessage = 3,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Request {
    Begin { kind: InputKind, bytes: u32 },
    Write { offset: u32, bytes: u32 },
    Commit,
    StartReceive { maximum: u32 },
    StepReceive,
    Read { offset: u32, maximum: u32 },
    Close,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Response {
    Ready,
    NeedInput { maximum: u32 },
    Continue,
    Blob { kind: OutputKind, bytes: u32 },
    Chunk { offset: u32, bytes: u32, total: u32 },
    Refused(Refusal),
}
fn encode(e: Envelope, tag: u16, args: [u32; 4], out: &mut [u8]) -> Result<(), Refusal> {
    if out.len() < BYTES || e.instance == 0 || e.sequence == 0 {
        return Err(Refusal::Malformed);
    }
    out[..BYTES].fill(0);
    out[..4].copy_from_slice(b"CNET");
    out[4..6].copy_from_slice(&1u16.to_le_bytes());
    out[6..8].copy_from_slice(&tag.to_le_bytes());
    out[8..12].copy_from_slice(&(BYTES as u32).to_le_bytes());
    out[16..24].copy_from_slice(&e.instance.to_le_bytes());
    out[24..32].copy_from_slice(&e.sequence.to_le_bytes());
    for (i, arg) in args.into_iter().enumerate() {
        out[32 + i * 4..36 + i * 4].copy_from_slice(&arg.to_le_bytes());
    }
    Ok(())
}
fn decode(bytes: &[u8]) -> Result<(Envelope, u16, [u32; 4]), Refusal> {
    if bytes.len() != BYTES
        || &bytes[..4] != b"CNET"
        || bytes[4..6] != 1u16.to_le_bytes()
        || bytes[8..12] != (BYTES as u32).to_le_bytes()
        || bytes[12..16] != [0; 4]
    {
        return Err(Refusal::Malformed);
    }
    let e = Envelope {
        instance: u64::from_le_bytes(bytes[16..24].try_into().map_err(|_| Refusal::Malformed)?),
        sequence: u64::from_le_bytes(bytes[24..32].try_into().map_err(|_| Refusal::Malformed)?),
    };
    if e.instance == 0 || e.sequence == 0 {
        return Err(Refusal::Malformed);
    }
    let mut a = [0; 4];
    for (i, arg) in a.iter_mut().enumerate() {
        *arg = u32::from_le_bytes(
            bytes[32 + i * 4..36 + i * 4]
                .try_into()
                .map_err(|_| Refusal::Malformed)?,
        );
    }
    Ok((
        e,
        u16::from_le_bytes(bytes[6..8].try_into().map_err(|_| Refusal::Malformed)?),
        a,
    ))
}
fn input(value: u32) -> Result<InputKind, Refusal> {
    match value {
        1 => Ok(InputKind::Initialize),
        2 => Ok(InputKind::Handshake),
        3 => Ok(InputKind::SendBinary),
        4 => Ok(InputKind::Receive),
        _ => Err(Refusal::Malformed),
    }
}
fn output(value: u32) -> Result<OutputKind, Refusal> {
    match value {
        1 => Ok(OutputKind::Upgrade),
        2 => Ok(OutputKind::WireFrame),
        3 => Ok(OutputKind::BinaryMessage),
        _ => Err(Refusal::Malformed),
    }
}
pub fn encode_request(e: Envelope, r: Request, out: &mut [u8]) -> Result<(), Refusal> {
    let (tag, a) = match r {
        Request::Begin { kind, bytes } => (1, [kind as u32, bytes, 0, 0]),
        Request::Write { offset, bytes } => (2, [offset, bytes, 0, 0]),
        Request::Commit => (3, [0; 4]),
        Request::StartReceive { maximum } => (4, [maximum, 0, 0, 0]),
        Request::StepReceive => (5, [0; 4]),
        Request::Read { offset, maximum } => (6, [offset, maximum, 0, 0]),
        Request::Close => (7, [0; 4]),
    };
    encode(e, tag, a, out)?;
    decode_request(&out[..BYTES]).map(|_| ())
}
pub fn decode_request(bytes: &[u8]) -> Result<(Envelope, Request), Refusal> {
    let (e, tag, a) = decode(bytes)?;
    let r = match (tag, a) {
        (1, [kind, bytes, 0, 0]) if bytes > 0 && bytes as usize <= MAX_BLOB => Request::Begin {
            kind: input(kind)?,
            bytes,
        },
        (2, [offset, bytes, 0, 0])
            if bytes > 0
                && bytes as usize <= TRANSFER_BYTES
                && offset
                    .checked_add(bytes)
                    .is_some_and(|end| end as usize <= MAX_BLOB) =>
        {
            Request::Write { offset, bytes }
        }
        (3, [0, 0, 0, 0]) => Request::Commit,
        (4, [maximum, 0, 0, 0])
            if maximum > 0 && maximum as usize <= MAXIMUM_BINARY_MESSAGE_BYTES =>
        {
            Request::StartReceive { maximum }
        }
        (5, [0, 0, 0, 0]) => Request::StepReceive,
        (6, [offset, maximum, 0, 0])
            if maximum > 0 && maximum as usize <= TRANSFER_BYTES && offset as usize <= MAX_BLOB =>
        {
            Request::Read { offset, maximum }
        }
        (7, [0, 0, 0, 0]) => Request::Close,
        _ => return Err(Refusal::Malformed),
    };
    Ok((e, r))
}
fn refusal_code(error: Refusal) -> u32 {
    match error {
        Refusal::Bounds => 1,
        Refusal::Malformed => 2,
        Refusal::Order => 3,
        Refusal::Capacity => 4,
        Refusal::WebSocket(error) => 256 + error as u32,
    }
}
fn refusal(code: u32) -> Result<Refusal, Refusal> {
    Ok(match code {
        1 => Refusal::Bounds,
        2 => Refusal::Malformed,
        3 => Refusal::Order,
        4 => Refusal::Capacity,
        257 => Refusal::WebSocket(WebSocketError::InvalidDescriptor),
        258 => Refusal::WebSocket(WebSocketError::RequestTooLarge),
        259 => Refusal::WebSocket(WebSocketError::ResponseTooLarge),
        260 => Refusal::WebSocket(WebSocketError::HandshakeWrite),
        261 => Refusal::WebSocket(WebSocketError::HandshakeRead),
        262 => Refusal::WebSocket(WebSocketError::HandshakeAuthentication),
        263 => Refusal::WebSocket(WebSocketError::FrameWrite),
        264 => Refusal::WebSocket(WebSocketError::FrameRead),
        265 => Refusal::WebSocket(WebSocketError::UnexpectedFrame),
        266 => Refusal::WebSocket(WebSocketError::Close),
        _ => return Err(Refusal::Malformed),
    })
}
pub fn encode_response(e: Envelope, r: Response, out: &mut [u8]) -> Result<(), Refusal> {
    let (tag, a) = match r {
        Response::Ready => (0x8001, [0; 4]),
        Response::NeedInput { maximum } => (0x8002, [maximum, 0, 0, 0]),
        Response::Continue => (0x8003, [0; 4]),
        Response::Blob { kind, bytes } => (0x8004, [kind as u32, bytes, 0, 0]),
        Response::Chunk {
            offset,
            bytes,
            total,
        } => (0x8005, [offset, bytes, total, 0]),
        Response::Refused(error) => (0x8fff, [refusal_code(error), 0, 0, 0]),
    };
    encode(e, tag, a, out)?;
    decode_response(&out[..BYTES]).map(|_| ())
}
pub fn decode_response(bytes: &[u8]) -> Result<(Envelope, Response), Refusal> {
    let (e, tag, a) = decode(bytes)?;
    let r = match (tag, a) {
        (0x8001, [0, 0, 0, 0]) => Response::Ready,
        (0x8002, [maximum, 0, 0, 0]) if maximum > 0 && maximum as usize <= crate::FRAME_BYTES => {
            Response::NeedInput { maximum }
        }
        (0x8003, [0, 0, 0, 0]) => Response::Continue,
        (0x8004, [kind, bytes, 0, 0])
            if bytes as usize <= MAX_BLOB
                && (bytes > 0 || kind == OutputKind::BinaryMessage as u32) =>
        {
            Response::Blob {
                kind: output(kind)?,
                bytes,
            }
        }
        (0x8005, [offset, bytes, total, 0])
            if bytes > 0
                && bytes as usize <= TRANSFER_BYTES
                && total as usize <= MAX_BLOB
                && offset.checked_add(bytes).is_some_and(|end| end <= total) =>
        {
            Response::Chunk {
                offset,
                bytes,
                total,
            }
        }
        (0x8fff, [code, 0, 0, 0]) => Response::Refused(refusal(code)?),
        _ => return Err(Refusal::Malformed),
    };
    Ok((e, r))
}
