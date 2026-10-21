//! Fixed little-endian frame/control envelopes; transfer bytes are never pointers.
use crate::Refusal;
pub const BYTES: usize = 64;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Envelope {
    pub instance: u64,
    pub sequence: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Request {
    Init,
    Poll { millis: u64, ingress: u32 },
    TakeFrame { frame: u32 },
    AckFrame { frame: u32 },
    Write { bytes: u32 },
    Read { maximum: u32 },
    Status,
    Close,
    Abort,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Response {
    Ready,
    Status {
        state: u32,
        receive: u32,
        send: u32,
        frames: u32,
        next_frame: u32,
        polls: u32,
        flags: u32,
    },
    Frame {
        frame: u32,
        bytes: u32,
    },
    Written {
        bytes: u32,
    },
    Read {
        bytes: u32,
    },
    Refused(Refusal),
}
fn encode(e: Envelope, tag: u32, args: [u32; 8], out: &mut [u8]) -> Result<(), Refusal> {
    if out.len() < BYTES || e.instance == 0 || e.sequence == 0 {
        return Err(Refusal::Malformed);
    }
    out[..BYTES].fill(0);
    out[..4].copy_from_slice(b"CTCP");
    out[4..6].copy_from_slice(&1u16.to_le_bytes());
    out[6..8].copy_from_slice(&(tag as u16).to_le_bytes());
    out[8..12].copy_from_slice(&(BYTES as u32).to_le_bytes());
    out[16..24].copy_from_slice(&e.instance.to_le_bytes());
    out[24..32].copy_from_slice(&e.sequence.to_le_bytes());
    for (i, v) in args.into_iter().enumerate() {
        out[32 + i * 4..36 + i * 4].copy_from_slice(&v.to_le_bytes());
    }
    Ok(())
}
fn decode(b: &[u8]) -> Result<(Envelope, u32, [u32; 8]), Refusal> {
    if b.len() != BYTES
        || &b[..4] != b"CTCP"
        || b[4..6] != 1u16.to_le_bytes()
        || b[8..12] != (BYTES as u32).to_le_bytes()
        || b[12..16] != [0; 4]
    {
        return Err(Refusal::Malformed);
    }
    let e = Envelope {
        instance: u64::from_le_bytes(b[16..24].try_into().map_err(|_| Refusal::Malformed)?),
        sequence: u64::from_le_bytes(b[24..32].try_into().map_err(|_| Refusal::Malformed)?),
    };
    if e.instance == 0 || e.sequence == 0 {
        return Err(Refusal::Malformed);
    }
    let mut a = [0; 8];
    for (i, v) in a.iter_mut().enumerate() {
        *v = u32::from_le_bytes(
            b[32 + i * 4..36 + i * 4]
                .try_into()
                .map_err(|_| Refusal::Malformed)?,
        );
    }
    Ok((e, u16::from_le_bytes([b[6], b[7]]) as u32, a))
}
pub fn encode_request(e: Envelope, r: Request, out: &mut [u8]) -> Result<(), Refusal> {
    let (tag, a) = match r {
        Request::Init => (1, [0; 8]),
        Request::Poll { millis, ingress } => (
            2,
            [millis as u32, (millis >> 32) as u32, ingress, 0, 0, 0, 0, 0],
        ),
        Request::TakeFrame { frame } => (3, [frame, 0, 0, 0, 0, 0, 0, 0]),
        Request::AckFrame { frame } => (4, [frame, 0, 0, 0, 0, 0, 0, 0]),
        Request::Write { bytes } => (5, [bytes, 0, 0, 0, 0, 0, 0, 0]),
        Request::Read { maximum } => (6, [maximum, 0, 0, 0, 0, 0, 0, 0]),
        Request::Status => (7, [0; 8]),
        Request::Close => (8, [0; 8]),
        Request::Abort => (9, [0; 8]),
    };
    encode(e, tag, a, out)
}
pub fn decode_request(b: &[u8]) -> Result<(Envelope, Request), Refusal> {
    let (e, t, a) = decode(b)?;
    let r = match t {
        1 => Request::Init,
        2 => Request::Poll {
            millis: u64::from(a[0]) | (u64::from(a[1]) << 32),
            ingress: a[2],
        },
        3 => Request::TakeFrame { frame: a[0] },
        4 => Request::AckFrame { frame: a[0] },
        5 => Request::Write { bytes: a[0] },
        6 => Request::Read { maximum: a[0] },
        7 => Request::Status,
        8 => Request::Close,
        9 => Request::Abort,
        _ => return Err(Refusal::Malformed),
    };
    let mut canonical = [0; BYTES];
    encode_request(e, r, &mut canonical)?;
    if canonical != b {
        return Err(Refusal::Malformed);
    }
    Ok((e, r))
}
pub fn encode_response(e: Envelope, r: Response, out: &mut [u8]) -> Result<(), Refusal> {
    let (t, a) = match r {
        Response::Ready => (101, [0; 8]),
        Response::Status {
            state,
            receive,
            send,
            frames,
            next_frame,
            polls,
            flags,
        } => (
            102,
            [state, receive, send, frames, next_frame, polls, flags, 0],
        ),
        Response::Frame { frame, bytes } => (103, [frame, bytes, 0, 0, 0, 0, 0, 0]),
        Response::Written { bytes } => (104, [bytes, 0, 0, 0, 0, 0, 0, 0]),
        Response::Read { bytes } => (105, [bytes, 0, 0, 0, 0, 0, 0, 0]),
        Response::Refused(r) => (106, [r as u32, 0, 0, 0, 0, 0, 0, 0]),
    };
    encode(e, t, a, out)
}
pub fn decode_response(b: &[u8]) -> Result<(Envelope, Response), Refusal> {
    let (e, t, a) = decode(b)?;
    let r = match t {
        101 => Response::Ready,
        102 if a[0] <= 10
            && a[1] <= 4096
            && a[2] <= 4096
            && a[3] <= 4
            && a[6] <= 15
            && ((a[3] == 0) == (a[4] == 0)) =>
        {
            Response::Status {
                state: a[0],
                receive: a[1],
                send: a[2],
                frames: a[3],
                next_frame: a[4],
                polls: a[5],
                flags: a[6],
            }
        }
        103 if a[0] != 0 && a[1] > 0 && a[1] <= 1514 => Response::Frame {
            frame: a[0],
            bytes: a[1],
        },
        104 if a[0] > 0 && a[0] <= 4096 => Response::Written { bytes: a[0] },
        105 if a[0] <= 4096 => Response::Read { bytes: a[0] },
        106 => Response::Refused(match a[0] {
            1 => Refusal::Bounds,
            2 => Refusal::Malformed,
            3 => Refusal::Order,
            4 => Refusal::Capacity,
            5 => Refusal::Connect,
            6 => Refusal::Send,
            7 => Refusal::Receive,
            8 => Refusal::Closed,
            9 => Refusal::Clock,
            _ => return Err(Refusal::Malformed),
        }),
        _ => return Err(Refusal::Malformed),
    };
    let mut canonical = [0; BYTES];
    encode_response(e, r, &mut canonical)?;
    if canonical != b {
        return Err(Refusal::Malformed);
    }
    Ok((e, r))
}
