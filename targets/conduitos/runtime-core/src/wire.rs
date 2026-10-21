//! Versioned explicit little-endian transport; never Rust layouts or pointers.
pub mod admission;
mod codec;
mod errors;
use codec::*;
use conduit_kernel::{
    scheduler::{HostCallRequest, SchedulerError, SchedulerStatus},
    *,
};
pub const EXCHANGE_BYTES: usize = 4096;
const HEADER: usize = 32;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Malformed;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Envelope {
    pub instance: u64,
    pub request: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Request {
    Init {
        bytes: u32,
    },
    Step,
    Next,
    Store {
        bytes: u32,
    },
    Complete {
        node: NodeId,
        request: RequestId,
        outcome: HostCallOutcome,
    },
    Cancel,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Response {
    Ready,
    Status(SchedulerStatus),
    Next(Option<HostCallRequest>),
    Stored(ValueRef),
    Ack,
    Cancelled,
    Failed(SchedulerError),
}
impl Request {
    pub fn tag(self) -> u16 {
        match self {
            Self::Init { .. } => 1,
            Self::Step => 2,
            Self::Next => 3,
            Self::Store { .. } => 4,
            Self::Complete { .. } => 5,
            Self::Cancel => 6,
        }
    }
}
impl Response {
    pub fn matches(self, request: Request) -> bool {
        matches!(self, Self::Failed(_))
            || matches!(
                (request, self),
                (Request::Init { .. }, Self::Ready)
                    | (Request::Step, Self::Status(_))
                    | (Request::Next, Self::Next(_))
                    | (Request::Store { .. }, Self::Stored(_))
                    | (Request::Complete { .. }, Self::Ack)
                    | (Request::Cancel, Self::Cancelled)
            )
    }
}
fn begin(bytes: &mut [u8], e: Envelope, tag: u16) -> Result<Writer<'_>, Malformed> {
    if e.instance == 0 || e.request == 0 {
        return Err(Malformed);
    }
    let mut w = Writer { bytes, at: 0 };
    w.raw(b"CRUN")?;
    w.u16(1)?;
    w.u16(tag)?;
    w.u32(0)?;
    w.u32(0)?;
    w.u64(e.instance)?;
    w.u64(e.request)?;
    Ok(w)
}
fn end(w: Writer<'_>) -> Result<usize, Malformed> {
    let len = u32::try_from(w.at).map_err(|_| Malformed)?;
    w.bytes
        .get_mut(8..12)
        .ok_or(Malformed)?
        .copy_from_slice(&len.to_le_bytes());
    Ok(w.at)
}
fn read(bytes: &[u8]) -> Result<(Envelope, u16, Reader<'_>), Malformed> {
    if bytes.len() < HEADER || bytes.len() > EXCHANGE_BYTES {
        return Err(Malformed);
    }
    let mut r = Reader { bytes, at: 0 };
    if r.raw(4)? != b"CRUN" || r.u16()? != 1 {
        return Err(Malformed);
    }
    let tag = r.u16()?;
    if r.u32()? as usize != bytes.len() || r.u32()? != 0 {
        return Err(Malformed);
    }
    let e = Envelope {
        instance: r.u64()?,
        request: r.u64()?,
    };
    if e.instance == 0 || e.request == 0 {
        return Err(Malformed);
    }
    Ok((e, tag, r))
}
pub fn encoded_len(bytes: &[u8]) -> Result<usize, Malformed> {
    Ok(u32::from_le_bytes(
        bytes
            .get(8..12)
            .ok_or(Malformed)?
            .try_into()
            .map_err(|_| Malformed)?,
    ) as usize)
}
pub fn encode_request(e: Envelope, request: Request, bytes: &mut [u8]) -> Result<usize, Malformed> {
    let mut w = begin(bytes, e, request.tag())?;
    match request {
        Request::Init { bytes } | Request::Store { bytes } => w.u32(bytes)?,
        Request::Complete {
            node,
            request,
            outcome,
        } => {
            w.u16(node.0)?;
            w.u32(request.0)?;
            outcome_write(&mut w, outcome)?;
        }
        _ => {}
    }
    end(w)
}
pub fn decode_request(bytes: &[u8]) -> Result<(Envelope, Request), Malformed> {
    let (e, tag, mut r) = read(bytes)?;
    let request = match tag {
        1 => Request::Init { bytes: r.u32()? },
        2 => Request::Step,
        3 => Request::Next,
        4 => Request::Store { bytes: r.u32()? },
        5 => Request::Complete {
            node: NodeId(r.u16()?),
            request: RequestId(r.u32()?),
            outcome: outcome_read(&mut r)?,
        },
        6 => Request::Cancel,
        _ => return Err(Malformed),
    };
    r.finish()?;
    Ok((e, request))
}
pub fn encode_response(
    e: Envelope,
    response: Response,
    gap: Option<SignRetentionGap>,
    bytes: &mut [u8],
) -> Result<usize, Malformed> {
    let tag = match response {
        Response::Ready => 0x8001,
        Response::Status(_) => 0x8002,
        Response::Next(_) => 0x8003,
        Response::Stored(_) => 0x8004,
        Response::Ack => 0x8005,
        Response::Cancelled => 0x8006,
        Response::Failed(_) => 0x8fff,
    };
    let mut w = begin(bytes, e, tag)?;
    w.u8(u8::from(gap.is_some()))?;
    if let Some(g) = gap {
        w.u32(g.first_sequence)?;
        w.u32(g.last_sequence)?;
        w.u32(g.entries)?;
    }
    match response {
        Response::Status(status) => match status {
            SchedulerStatus::Progress { node } => {
                w.u8(0)?;
                w.u16(node.0)?
            }
            SchedulerStatus::Idle => w.u8(1)?,
            SchedulerStatus::Drained => w.u8(2)?,
            SchedulerStatus::Cancelled => w.u8(3)?,
        },
        Response::Next(request) => {
            w.u8(u8::from(request.is_some()))?;
            if let Some(request) = request {
                w.u16(request.node.0)?;
                w.u32(request.request.0)?;
                w.u16(request.call.0)?;
                bounded_write(&mut w, request.input)?;
            }
        }
        Response::Stored(value) => value_write(&mut w, value)?,
        Response::Failed(error) => errors::write(&mut w, error)?,
        _ => {}
    }
    end(w)
}
pub fn decode_response(
    bytes: &[u8],
) -> Result<(Envelope, Response, Option<SignRetentionGap>), Malformed> {
    let (e, tag, mut r) = read(bytes)?;
    let gap = if r.flag()? {
        let g = SignRetentionGap {
            first_sequence: r.u32()?,
            last_sequence: r.u32()?,
            entries: r.u32()?,
        };
        if g.entries == 0 || g.first_sequence > g.last_sequence {
            return Err(Malformed);
        }
        Some(g)
    } else {
        None
    };
    let response = match tag {
        0x8001 => Response::Ready,
        0x8002 => Response::Status(match r.u8()? {
            0 => SchedulerStatus::Progress {
                node: NodeId(r.u16()?),
            },
            1 => SchedulerStatus::Idle,
            2 => SchedulerStatus::Drained,
            3 => SchedulerStatus::Cancelled,
            _ => return Err(Malformed),
        }),
        0x8003 => Response::Next(if r.flag()? {
            Some(HostCallRequest {
                node: NodeId(r.u16()?),
                request: RequestId(r.u32()?),
                call: HostCallId(r.u16()?),
                input: bounded_read(&mut r)?,
            })
        } else {
            None
        }),
        0x8004 => Response::Stored(value_read(&mut r)?),
        0x8005 => Response::Ack,
        0x8006 => Response::Cancelled,
        0x8fff => Response::Failed(errors::read(&mut r)?),
        _ => return Err(Malformed),
    };
    r.finish()?;
    Ok((e, response, gap))
}
fn value_write(w: &mut Writer<'_>, value: ValueRef) -> Result<(), Malformed> {
    w.u16(value.slot)?;
    w.u16(value.generation)?;
    w.u32(value.byte_len)
}
fn value_read(r: &mut Reader<'_>) -> Result<ValueRef, Malformed> {
    Ok(ValueRef {
        slot: r.u16()?,
        generation: r.u16()?,
        byte_len: r.u32()?,
    })
}
fn bounded_write(w: &mut Writer<'_>, value: BoundedValueRef) -> Result<(), Malformed> {
    value_write(w, value.value)?;
    w.u32(value.admitted_bytes)
}
fn bounded_read(r: &mut Reader<'_>) -> Result<BoundedValueRef, Malformed> {
    BoundedValueRef::new(value_read(r)?, r.u32()?).map_err(|_| Malformed)
}
fn outcome_write(w: &mut Writer<'_>, outcome: HostCallOutcome) -> Result<(), Malformed> {
    w.u8(match outcome.disposition {
        HostCallDisposition::Completed => 0,
        HostCallDisposition::Denied => 1,
        HostCallDisposition::Cancelled => 2,
        HostCallDisposition::Failed => 3,
    })?;
    w.u8(u8::from(outcome.output.is_some()))?;
    if let Some(output) = outcome.output {
        bounded_write(w, output)?;
    }
    w.u8(u8::from(outcome.failure.is_some()))?;
    if let Some(f) = outcome.failure {
        errors::failure_write(w, f.code)?;
        w.u16(f.detail)?;
    }
    Ok(())
}
fn outcome_read(r: &mut Reader<'_>) -> Result<HostCallOutcome, Malformed> {
    let disposition = match r.u8()? {
        0 => HostCallDisposition::Completed,
        1 => HostCallDisposition::Denied,
        2 => HostCallDisposition::Cancelled,
        3 => HostCallDisposition::Failed,
        _ => return Err(Malformed),
    };
    let output = if r.flag()? {
        Some(bounded_read(r)?)
    } else {
        None
    };
    let failure = if r.flag()? {
        Some(Failure {
            code: errors::failure_read(r)?,
            detail: r.u16()?,
        })
    } else {
        None
    };
    Ok(HostCallOutcome {
        disposition,
        output,
        failure,
    })
}
