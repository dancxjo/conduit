//! Closed correlated chunk transport, with no address-valued fields.
use crate::{MAX_BLOB, data::Refusal, memory::TRANSFER_BYTES};
pub const BYTES: usize = 48;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Envelope {
    pub instance: u64,
    pub sequence: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Request {
    Begin {
        bytes: u32,
    },
    Write {
        offset: u32,
        bytes: u32,
    },
    Commit,
    /// Validate a fully written target command before its typed Commit entry.
    Preflight,
    /// Ordered phases of an immutable retained Graph reply.
    GraphSerialize,
    GraphValidate,
    GraphPack {
        offset: u32,
    },
    Read {
        offset: u32,
        maximum: u32,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Response {
    Ready,
    Blob { bytes: u32 },
    Chunk { offset: u32, bytes: u32, total: u32 },
    Refused(Refusal),
}
fn encode(e: Envelope, tag: u16, args: [u32; 4], out: &mut [u8]) -> Result<(), Refusal> {
    if out.len() < BYTES || e.instance == 0 || e.sequence == 0 {
        return Err(Refusal::Malformed);
    }
    out[..BYTES].fill(0);
    out[..4].copy_from_slice(b"CAPP");
    out[4..6].copy_from_slice(&1u16.to_le_bytes());
    out[6..8].copy_from_slice(&tag.to_le_bytes());
    out[8..12].copy_from_slice(&(BYTES as u32).to_le_bytes());
    out[16..24].copy_from_slice(&e.instance.to_le_bytes());
    out[24..32].copy_from_slice(&e.sequence.to_le_bytes());
    for (index, arg) in args.into_iter().enumerate() {
        out[32 + index * 4..36 + index * 4].copy_from_slice(&arg.to_le_bytes());
    }
    Ok(())
}
fn decode(input: &[u8]) -> Result<(Envelope, u16, [u32; 4]), Refusal> {
    if input.len() != BYTES
        || &input[..4] != b"CAPP"
        || input[4..6] != 1u16.to_le_bytes()
        || input[8..12] != (BYTES as u32).to_le_bytes()
        || input[12..16] != [0; 4]
    {
        return Err(Refusal::Malformed);
    }
    let e = Envelope {
        instance: u64::from_le_bytes(input[16..24].try_into().map_err(|_| Refusal::Malformed)?),
        sequence: u64::from_le_bytes(input[24..32].try_into().map_err(|_| Refusal::Malformed)?),
    };
    if e.instance == 0 || e.sequence == 0 {
        return Err(Refusal::Malformed);
    }
    let mut args = [0; 4];
    for (index, arg) in args.iter_mut().enumerate() {
        *arg = u32::from_le_bytes(
            input[32 + index * 4..36 + index * 4]
                .try_into()
                .map_err(|_| Refusal::Malformed)?,
        );
    }
    Ok((
        e,
        u16::from_le_bytes(input[6..8].try_into().map_err(|_| Refusal::Malformed)?),
        args,
    ))
}
pub fn encode_request(e: Envelope, r: Request, out: &mut [u8]) -> Result<(), Refusal> {
    let (tag, args) = match r {
        Request::Begin { bytes } => (1, [bytes, 0, 0, 0]),
        Request::Write { offset, bytes } => (2, [offset, bytes, 0, 0]),
        Request::Commit => (3, [0; 4]),
        Request::Preflight => (5, [0; 4]),
        Request::GraphSerialize => (6, [0; 4]),
        Request::GraphValidate => (7, [0; 4]),
        Request::GraphPack { offset } => (8, [offset, 0, 0, 0]),
        Request::Read { offset, maximum } => (4, [offset, maximum, 0, 0]),
    };
    encode(e, tag, args, out)
}
pub fn decode_request(input: &[u8]) -> Result<(Envelope, Request), Refusal> {
    let (e, tag, a) = decode(input)?;
    let r = match (tag, a) {
        (1, [bytes, 0, 0, 0]) if bytes > 0 && bytes as usize <= MAX_BLOB => {
            Request::Begin { bytes }
        }
        (2, [offset, bytes, 0, 0]) if bytes > 0 && bytes as usize <= TRANSFER_BYTES => {
            Request::Write { offset, bytes }
        }
        (3, [0, 0, 0, 0]) => Request::Commit,
        (5, [0, 0, 0, 0]) => Request::Preflight,
        (6, [0, 0, 0, 0]) => Request::GraphSerialize,
        (7, [0, 0, 0, 0]) => Request::GraphValidate,
        (8, [offset, 0, 0, 0]) => Request::GraphPack { offset },
        (4, [offset, maximum, 0, 0]) if maximum > 0 && maximum as usize <= TRANSFER_BYTES => {
            Request::Read { offset, maximum }
        }
        _ => return Err(Refusal::Malformed),
    };
    Ok((e, r))
}
pub fn encode_response(e: Envelope, r: Response, out: &mut [u8]) -> Result<(), Refusal> {
    let (tag, args) = match r {
        Response::Ready => (0x8001, [0; 4]),
        Response::Blob { bytes } => (0x8002, [bytes, 0, 0, 0]),
        Response::Chunk {
            offset,
            bytes,
            total,
        } => (0x8003, [offset, bytes, total, 0]),
        Response::Refused(error) => (
            0x8fff,
            [
                match error {
                    Refusal::Bounds => 1,
                    Refusal::Malformed => 2,
                    Refusal::Order => 3,
                    Refusal::Capacity => 4,
                    Refusal::Application => 5,
                },
                0,
                0,
                0,
            ],
        ),
    };
    encode(e, tag, args, out)
}
pub fn decode_response(input: &[u8]) -> Result<(Envelope, Response), Refusal> {
    let (e, tag, a) = decode(input)?;
    let r = match (tag, a) {
        (0x8001, [0, 0, 0, 0]) => Response::Ready,
        (0x8002, [bytes, 0, 0, 0]) if bytes > 0 && bytes as usize <= MAX_BLOB => {
            Response::Blob { bytes }
        }
        (0x8003, [offset, bytes, total, 0])
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
        (0x8fff, [code, 0, 0, 0]) => Response::Refused(match code {
            1 => Refusal::Bounds,
            2 => Refusal::Malformed,
            3 => Refusal::Order,
            4 => Refusal::Capacity,
            5 => Refusal::Application,
            _ => return Err(Refusal::Malformed),
        }),
        _ => return Err(Refusal::Malformed),
    };
    Ok((e, r))
}
