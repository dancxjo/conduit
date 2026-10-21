//! Canonical fixed descriptors. Raw transfer bytes never contain guest addresses.
use crate::{
    CERTIFICATE_BYTES, Failure, HOST_BYTES, Operation, RECEIVE_BYTES, Refusal, TRANSMIT_BYTES,
    memory::TRANSFER_BYTES,
};
pub const BYTES: usize = 64;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Envelope {
    pub instance: u64,
    pub sequence: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Request {
    Init {
        host_bytes: u32,
        cert_bytes: u32,
    },
    Start {
        operation: u64,
        kind: Operation,
        bytes: u32,
    },
    Poll {
        operation: u64,
    },
    Provide {
        operation: u64,
        event: u64,
        bytes: u32,
    },
    Take {
        operation: u64,
        event: u64,
        offset: u32,
        maximum: u32,
    },
    Ack {
        operation: u64,
        event: u64,
    },
    Plaintext {
        operation: u64,
        offset: u32,
        maximum: u32,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Response {
    Ready,
    Started {
        operation: u64,
    },
    NeedRead {
        operation: u64,
        event: u64,
        maximum: u32,
    },
    NeedWrite {
        operation: u64,
        event: u64,
        bytes: u32,
    },
    NeedFlush {
        operation: u64,
        event: u64,
    },
    Chunk {
        operation: u64,
        event: u64,
        offset: u32,
        bytes: u32,
        total: u32,
    },
    Done {
        operation: u64,
        kind: Operation,
        bytes: u32,
    },
    Failed {
        operation: u64,
        error: Failure,
    },
    Refused(Refusal),
}
fn encode(
    e: Envelope,
    tag: u16,
    operation: u64,
    event: u64,
    a: [u32; 4],
    out: &mut [u8],
) -> Result<(), Refusal> {
    if out.len() < BYTES || e.instance == 0 || e.sequence == 0 {
        return Err(Refusal::Malformed);
    }
    out[..BYTES].fill(0);
    out[..4].copy_from_slice(b"CTLS");
    out[4..6].copy_from_slice(&1u16.to_le_bytes());
    out[6..8].copy_from_slice(&tag.to_le_bytes());
    out[8..12].copy_from_slice(&(BYTES as u32).to_le_bytes());
    out[16..24].copy_from_slice(&e.instance.to_le_bytes());
    out[24..32].copy_from_slice(&e.sequence.to_le_bytes());
    out[32..40].copy_from_slice(&operation.to_le_bytes());
    out[40..48].copy_from_slice(&event.to_le_bytes());
    for (i, arg) in a.into_iter().enumerate() {
        out[48 + i * 4..52 + i * 4].copy_from_slice(&arg.to_le_bytes());
    }
    Ok(())
}
fn decode(b: &[u8]) -> Result<(Envelope, u16, u64, u64, [u32; 4]), Refusal> {
    if b.len() != BYTES
        || &b[..4] != b"CTLS"
        || b[4..6] != 1u16.to_le_bytes()
        || b[8..12] != (BYTES as u32).to_le_bytes()
        || b[12..16] != [0; 4]
    {
        return Err(Refusal::Malformed);
    }
    let u64at = |at| -> Result<u64, Refusal> {
        Ok(u64::from_le_bytes(
            b[at..at + 8].try_into().map_err(|_| Refusal::Malformed)?,
        ))
    };
    let e = Envelope {
        instance: u64at(16)?,
        sequence: u64at(24)?,
    };
    if e.instance == 0 || e.sequence == 0 {
        return Err(Refusal::Malformed);
    }
    let mut a = [0; 4];
    for (i, arg) in a.iter_mut().enumerate() {
        *arg = u32::from_le_bytes(
            b[48 + i * 4..52 + i * 4]
                .try_into()
                .map_err(|_| Refusal::Malformed)?,
        );
    }
    Ok((
        e,
        u16::from_le_bytes(b[6..8].try_into().map_err(|_| Refusal::Malformed)?),
        u64at(32)?,
        u64at(40)?,
        a,
    ))
}
fn kind(n: u32) -> Result<Operation, Refusal> {
    match n {
        1 => Ok(Operation::Open),
        2 => Ok(Operation::Write),
        3 => Ok(Operation::Flush),
        4 => Ok(Operation::Read),
        5 => Ok(Operation::Close),
        _ => Err(Refusal::Malformed),
    }
}
fn failure(n: u32) -> Result<Failure, Refusal> {
    match n {
        1 => Ok(Failure::Authentication),
        2 => Ok(Failure::Handshake),
        3 => Ok(Failure::Write),
        4 => Ok(Failure::Read),
        5 => Ok(Failure::Close),
        _ => Err(Refusal::Malformed),
    }
}
fn refusal(n: u32) -> Result<Refusal, Refusal> {
    match n {
        1 => Ok(Refusal::Bounds),
        2 => Ok(Refusal::Malformed),
        3 => Ok(Refusal::Order),
        4 => Ok(Refusal::Capacity),
        _ => Err(Refusal::Malformed),
    }
}
pub fn encode_request(e: Envelope, r: Request, out: &mut [u8]) -> Result<(), Refusal> {
    let (t, o, v, a) = match r {
        Request::Init {
            host_bytes,
            cert_bytes,
        } => (1, 0, 0, [host_bytes, cert_bytes, 0, 0]),
        Request::Start {
            operation,
            kind,
            bytes,
        } => (2, operation, 0, [kind as u32, bytes, 0, 0]),
        Request::Poll { operation } => (3, operation, 0, [0; 4]),
        Request::Provide {
            operation,
            event,
            bytes,
        } => (4, operation, event, [bytes, 0, 0, 0]),
        Request::Take {
            operation,
            event,
            offset,
            maximum,
        } => (5, operation, event, [offset, maximum, 0, 0]),
        Request::Ack { operation, event } => (6, operation, event, [0; 4]),
        Request::Plaintext {
            operation,
            offset,
            maximum,
        } => (7, operation, 0, [offset, maximum, 0, 0]),
    };
    encode(e, t, o, v, a, out)?;
    decode_request(&out[..BYTES]).map(|_| ())
}
pub fn decode_request(b: &[u8]) -> Result<(Envelope, Request), Refusal> {
    let (e, t, o, v, a) = decode(b)?;
    let r = match (t, o, v, a) {
        (1, 0, 0, [host_bytes, cert_bytes, 0, 0])
            if host_bytes > 0
                && host_bytes as usize <= HOST_BYTES
                && cert_bytes > 0
                && cert_bytes as usize <= CERTIFICATE_BYTES =>
        {
            Request::Init {
                host_bytes,
                cert_bytes,
            }
        }
        (2, o, 0, [k, bytes, 0, 0]) if o > 0 => {
            let k = kind(k)?;
            if match k {
                Operation::Write | Operation::Read => bytes == 0 || bytes as usize > TRANSFER_BYTES,
                _ => bytes != 0,
            } {
                return Err(Refusal::Malformed);
            }
            Request::Start {
                operation: o,
                kind: k,
                bytes,
            }
        }
        (3, o, 0, [0, 0, 0, 0]) if o > 0 => Request::Poll { operation: o },
        (4, o, v, [bytes, 0, 0, 0])
            if o > 0 && v > 0 && bytes > 0 && bytes as usize <= TRANSFER_BYTES =>
        {
            Request::Provide {
                operation: o,
                event: v,
                bytes,
            }
        }
        (5, o, v, [offset, maximum, 0, 0])
            if o > 0
                && v > 0
                && maximum > 0
                && maximum as usize <= TRANSFER_BYTES
                && offset as usize <= TRANSMIT_BYTES =>
        {
            Request::Take {
                operation: o,
                event: v,
                offset,
                maximum,
            }
        }
        (6, o, v, [0, 0, 0, 0]) if o > 0 && v > 0 => Request::Ack {
            operation: o,
            event: v,
        },
        (7, o, 0, [offset, maximum, 0, 0])
            if o > 0
                && maximum > 0
                && maximum as usize <= TRANSFER_BYTES
                && offset as usize <= TRANSFER_BYTES =>
        {
            Request::Plaintext {
                operation: o,
                offset,
                maximum,
            }
        }
        _ => return Err(Refusal::Malformed),
    };
    Ok((e, r))
}
pub fn encode_response(e: Envelope, r: Response, out: &mut [u8]) -> Result<(), Refusal> {
    let (t, o, v, a) = match r {
        Response::Ready => (0x8001, 0, 0, [0; 4]),
        Response::Started { operation } => (0x8002, operation, 0, [0; 4]),
        Response::NeedRead {
            operation,
            event,
            maximum,
        } => (0x8003, operation, event, [maximum, 0, 0, 0]),
        Response::NeedWrite {
            operation,
            event,
            bytes,
        } => (0x8004, operation, event, [bytes, 0, 0, 0]),
        Response::NeedFlush { operation, event } => (0x8005, operation, event, [0; 4]),
        Response::Chunk {
            operation,
            event,
            offset,
            bytes,
            total,
        } => (0x8006, operation, event, [offset, bytes, total, 0]),
        Response::Done {
            operation,
            kind,
            bytes,
        } => (0x8007, operation, 0, [kind as u32, bytes, 0, 0]),
        Response::Failed { operation, error } => (0x8008, operation, 0, [error as u32, 0, 0, 0]),
        Response::Refused(error) => (0x8fff, 0, 0, [error as u32, 0, 0, 0]),
    };
    encode(e, t, o, v, a, out)?;
    decode_response(&out[..BYTES]).map(|_| ())
}
pub fn decode_response(b: &[u8]) -> Result<(Envelope, Response), Refusal> {
    let (e, t, o, v, a) = decode(b)?;
    let r = match (t, o, v, a) {
        (0x8001, 0, 0, [0, 0, 0, 0]) => Response::Ready,
        (0x8002, o, 0, [0, 0, 0, 0]) if o > 0 => Response::Started { operation: o },
        (0x8003, o, v, [maximum, 0, 0, 0])
            if o > 0 && v > 0 && maximum > 0 && maximum as usize <= RECEIVE_BYTES =>
        {
            Response::NeedRead {
                operation: o,
                event: v,
                maximum,
            }
        }
        (0x8004, o, v, [bytes, 0, 0, 0])
            if o > 0 && v > 0 && bytes > 0 && bytes as usize <= TRANSMIT_BYTES =>
        {
            Response::NeedWrite {
                operation: o,
                event: v,
                bytes,
            }
        }
        (0x8005, o, v, [0, 0, 0, 0]) if o > 0 && v > 0 => Response::NeedFlush {
            operation: o,
            event: v,
        },
        (0x8006, o, v, [offset, bytes, total, 0])
            if o > 0
                && bytes > 0
                && bytes as usize <= TRANSFER_BYTES
                && total as usize <= RECEIVE_BYTES
                && offset.checked_add(bytes).is_some_and(|end| end <= total) =>
        {
            Response::Chunk {
                operation: o,
                event: v,
                offset,
                bytes,
                total,
            }
        }
        (0x8007, o, 0, [k, bytes, 0, 0]) if o > 0 => {
            let k = kind(k)?;
            if match k {
                Operation::Write | Operation::Read => bytes as usize > TRANSFER_BYTES,
                _ => bytes != 0,
            } {
                return Err(Refusal::Malformed);
            }
            Response::Done {
                operation: o,
                kind: k,
                bytes,
            }
        }
        (0x8008, o, 0, [error, 0, 0, 0]) if o > 0 => Response::Failed {
            operation: o,
            error: failure(error)?,
        },
        (0x8fff, 0, 0, [error, 0, 0, 0]) => Response::Refused(refusal(error)?),
        _ => return Err(Refusal::Malformed),
    };
    Ok((e, r))
}
