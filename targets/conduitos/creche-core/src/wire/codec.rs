use super::*;
const MAGIC: u32 = 0x48435243;
fn u16_at(b: &[u8], o: usize) -> Result<u16, Refusal> {
    Ok(u16::from_le_bytes(
        b.get(o..o + 2)
            .ok_or(Refusal::Malformed)?
            .try_into()
            .map_err(|_| Refusal::Malformed)?,
    ))
}
fn u32_at(b: &[u8], o: usize) -> Result<u32, Refusal> {
    Ok(u32::from_le_bytes(
        b.get(o..o + 4)
            .ok_or(Refusal::Malformed)?
            .try_into()
            .map_err(|_| Refusal::Malformed)?,
    ))
}
fn u64_at(b: &[u8], o: usize) -> Result<u64, Refusal> {
    Ok(u64::from_le_bytes(
        b.get(o..o + 8)
            .ok_or(Refusal::Malformed)?
            .try_into()
            .map_err(|_| Refusal::Malformed)?,
    ))
}
fn header(bytes: &[u8]) -> Result<(Envelope, u16, &[u8]), Refusal> {
    if bytes.len() < HEADER_BYTES
        || bytes.len() > EXCHANGE_BYTES
        || u32_at(bytes, 0)? != MAGIC
        || u16_at(bytes, 4)? != VERSION
        || u32_at(bytes, 8)? as usize != bytes.len()
        || u32_at(bytes, 12)? != 0
    {
        return Err(Refusal::Malformed);
    }
    let env = Envelope {
        instance: u64_at(bytes, 16)?,
        request: u64_at(bytes, 24)?,
    };
    if env.instance == 0 || env.request == 0 {
        return Err(Refusal::Malformed);
    }
    Ok((env, u16_at(bytes, 6)?, &bytes[HEADER_BYTES..]))
}
fn start(env: Envelope, op: u16, payload: usize, out: &mut [u8]) -> Result<&mut [u8], Refusal> {
    let length = HEADER_BYTES.checked_add(payload).ok_or(Refusal::Bounds)?;
    if env.instance == 0 || env.request == 0 || length > EXCHANGE_BYTES || length > out.len() {
        return Err(Refusal::Bounds);
    }
    let out = &mut out[..length];
    out.fill(0);
    out[..4].copy_from_slice(&MAGIC.to_le_bytes());
    out[4..6].copy_from_slice(&VERSION.to_le_bytes());
    out[6..8].copy_from_slice(&op.to_le_bytes());
    out[8..12].copy_from_slice(&(length as u32).to_le_bytes());
    out[16..24].copy_from_slice(&env.instance.to_le_bytes());
    out[24..32].copy_from_slice(&env.request.to_le_bytes());
    Ok(out)
}
fn exact(b: &[u8], n: usize) -> Result<(), Refusal> {
    if b.len() == n {
        Ok(())
    } else {
        Err(Refusal::Malformed)
    }
}
fn kind(b: u8) -> Result<InputKind, Refusal> {
    match b {
        1 => Ok(InputKind::Initialize),
        2 => Ok(InputKind::FaceBasis),
        3 => Ok(InputKind::FaceInteraction),
        4 => Ok(InputKind::NativeFace),
        5 => Ok(InputKind::NativeScene),
        6 => Ok(InputKind::HomeKey),
        7 => Ok(InputKind::HomeView),
        8 => Ok(InputKind::HomeScene),
        9 => Ok(InputKind::HomeContent),
        _ => Err(Refusal::Malformed),
    }
}
fn output(b: u8) -> Result<OutputKind, Refusal> {
    match b {
        1 => Ok(OutputKind::Snapshot),
        2 => Ok(OutputKind::Face),
        3 => Ok(OutputKind::Selection),
        4 => Ok(OutputKind::Native),
        5 => Ok(OutputKind::NativeFace),
        6 => Ok(OutputKind::NativeScene),
        7 => Ok(OutputKind::Home),
        8 => Ok(OutputKind::HomeView),
        9 => Ok(OutputKind::HomeScene),
        10 => Ok(OutputKind::HomeContent),
        _ => Err(Refusal::Malformed),
    }
}
fn event(b: u8) -> Result<Event, Refusal> {
    match b {
        1 => Ok(Event::Input),
        2 => Ok(Event::Change),
        3 => Ok(Event::Activate),
        _ => Err(Refusal::Malformed),
    }
}
fn refusal(b: u16) -> Result<Refusal, Refusal> {
    use Refusal::*;
    Ok(match b {
        1 => Bounds,
        2 => Malformed,
        3 => Order,
        4 => StaleInstance,
        5 => StaleRequest,
        6 => Capacity,
        7 => Projection,
        8 => Interaction,
        16 => NamingCatalog,
        17 => NamingUuid,
        18 => NamingSystem,
        19 => NamingBounds,
        20 => InvalidName,
        21 => InvalidInventory,
        22 => EmptySelection,
        23 => UnavailablePlot,
        24 => StaleFace,
        25 => RevisionExhausted,
        26 => UnknownChoice,
        27 => UnknownAction,
        28 => InvalidActionValue,
        29 => InvalidSearch,
        _ => return Err(Malformed),
    })
}
fn text(b: &[u8], limit: usize) -> Result<&str, Refusal> {
    if b.len() > limit {
        return Err(Refusal::Bounds);
    }
    core::str::from_utf8(b).map_err(|_| Refusal::Malformed)
}
pub fn decode_request(bytes: &[u8]) -> Result<(Envelope, Request<'_>), Refusal> {
    let (env, op, b) = header(bytes)?;
    let request = match op {
        1 => {
            exact(b, 5)?;
            let total = u32_at(b, 1)?;
            if total == 0 || total > MAX_BLOB_BYTES {
                return Err(Refusal::Bounds);
            }
            Request::Begin {
                kind: kind(b[0])?,
                total,
            }
        }
        2 => {
            exact(b, 12)?;
            let offset = u32_at(b, 0)?;
            let window_offset = u32_at(b, 4)?;
            let bytes = u32_at(b, 8)?;
            if bytes == 0
                || window_offset
                    .checked_add(bytes)
                    .is_none_or(|n| n > TRANSFER_BYTES)
            {
                return Err(Refusal::Bounds);
            }
            Request::Write {
                offset,
                window_offset,
                bytes,
            }
        }
        3 => {
            exact(b, 0)?;
            Request::Commit
        }
        4 => {
            if b.len() < 9 {
                return Err(Refusal::Malformed);
            }
            let a = usize::from(u16_at(b, 5)?);
            let v = usize::from(u16_at(b, 7)?);
            let end = 9usize
                .checked_add(a)
                .and_then(|n| n.checked_add(v))
                .ok_or(Refusal::Bounds)?;
            exact(b, end)?;
            Request::Apply {
                revision: u32_at(b, 0)?,
                event: event(b[4])?,
                action: text(&b[9..9 + a], 64)?,
                value: text(&b[9 + a..], 128)?,
            }
        }
        5 | 6 => {
            exact(b, 4)?;
            let revision = u32_at(b, 0)?;
            if op == 5 {
                Request::Snapshot { revision }
            } else {
                Request::Selection { revision }
            }
        }
        7 => {
            exact(b, 8)?;
            let maximum = u32_at(b, 4)?;
            if maximum == 0 || maximum > TRANSFER_BYTES {
                return Err(Refusal::Bounds);
            }
            Request::Read {
                offset: u32_at(b, 0)?,
                maximum,
            }
        }
        8 => {
            exact(b, 0)?;
            Request::AllowIdle
        }
        9 => {
            exact(b, 7)?;
            let event: [u8; 3] = b[4..7].try_into().map_err(|_| Refusal::Malformed)?;
            conduit_human::KeyEvent::decode(&event).map_err(|_| Refusal::Malformed)?;
            Request::NativeKey {
                revision: u32_at(b, 0)?,
                event,
            }
        }
        10 => {
            exact(b, 0)?;
            Request::HomeInit
        }
        11 => {
            exact(b, 5)?;
            if !(1..=2).contains(&b[4]) {
                return Err(Refusal::Malformed);
            }
            Request::HomeRefusal {
                revision: u32_at(b, 0)?,
                code: b[4],
            }
        }
        12 => {
            exact(b, 4)?;
            Request::HomeReset {
                revision: u32_at(b, 0)?,
            }
        }
        _ => return Err(Refusal::Malformed),
    };
    Ok((env, request))
}
pub fn encode_request(env: Envelope, r: Request<'_>, out: &mut [u8]) -> Result<usize, Refusal> {
    let (op, n) = match r {
        Request::Begin { .. } => (1, 5),
        Request::Write { .. } => (2, 12),
        Request::Commit => (3, 0),
        Request::AllowIdle => (8, 0),
        Request::HomeInit => (10, 0),
        Request::HomeReset { .. } => (12, 4),
        Request::HomeRefusal { .. } => (11, 5),
        Request::NativeKey { .. } => (9, 7),
        Request::Apply { action, value, .. } => {
            if action.len() > 64 || value.len() > 128 {
                return Err(Refusal::Bounds);
            }
            (4, 9 + action.len() + value.len())
        }
        Request::Snapshot { .. } => (5, 4),
        Request::Selection { .. } => (6, 4),
        Request::Read { .. } => (7, 8),
    };
    let out = start(env, op, n, out)?;
    let b = &mut out[HEADER_BYTES..];
    match r {
        Request::Begin { kind, total } => {
            b[0] = kind as u8;
            b[1..5].copy_from_slice(&total.to_le_bytes());
        }
        Request::Write {
            offset,
            window_offset,
            bytes,
        } => {
            b[..4].copy_from_slice(&offset.to_le_bytes());
            b[4..8].copy_from_slice(&window_offset.to_le_bytes());
            b[8..12].copy_from_slice(&bytes.to_le_bytes());
        }
        Request::Commit | Request::AllowIdle | Request::HomeInit => {}
        Request::HomeReset { revision } => {
            b[..4].copy_from_slice(&revision.to_le_bytes());
        }
        Request::HomeRefusal { revision, code } => {
            b[..4].copy_from_slice(&revision.to_le_bytes());
            b[4] = code;
        }
        Request::NativeKey { revision, event } => {
            b[..4].copy_from_slice(&revision.to_le_bytes());
            b[4..].copy_from_slice(&event);
        }
        Request::Apply {
            revision,
            event,
            action,
            value,
        } => {
            if action.len() > 64 || value.len() > 128 {
                return Err(Refusal::Bounds);
            }
            b[..4].copy_from_slice(&revision.to_le_bytes());
            b[4] = event as u8;
            b[5..7].copy_from_slice(&(action.len() as u16).to_le_bytes());
            b[7..9].copy_from_slice(&(value.len() as u16).to_le_bytes());
            b[9..9 + action.len()].copy_from_slice(action.as_bytes());
            b[9 + action.len()..].copy_from_slice(value.as_bytes());
        }
        Request::Snapshot { revision } | Request::Selection { revision } => {
            b.copy_from_slice(&revision.to_le_bytes())
        }
        Request::Read { offset, maximum } => {
            b[..4].copy_from_slice(&offset.to_le_bytes());
            b[4..8].copy_from_slice(&maximum.to_le_bytes());
        }
    }
    decode_request(out)?;
    Ok(out.len())
}
pub fn encode_response(env: Envelope, r: Response, out: &mut [u8]) -> Result<usize, Refusal> {
    let (op, n) = match r {
        Response::Ready { .. } => (0x8001, 4),
        Response::Blob { .. } => (0x8002, 9),
        Response::Chunk { .. } => (0x8003, 13),
        Response::Refused(_) => (0xffff, 2),
    };
    let out = start(env, op, n, out)?;
    let b = &mut out[HEADER_BYTES..];
    match r {
        Response::Ready { revision } => b.copy_from_slice(&revision.to_le_bytes()),
        Response::Blob {
            kind,
            revision,
            bytes,
        } => {
            b[0] = kind as u8;
            b[1..5].copy_from_slice(&revision.to_le_bytes());
            b[5..9].copy_from_slice(&bytes.to_le_bytes());
        }
        Response::Chunk {
            offset,
            bytes,
            total,
            complete,
        } => {
            b[..4].copy_from_slice(&offset.to_le_bytes());
            b[4..8].copy_from_slice(&bytes.to_le_bytes());
            b[8..12].copy_from_slice(&total.to_le_bytes());
            b[12] = u8::from(complete);
        }
        Response::Refused(error) => b.copy_from_slice(&(error as u16).to_le_bytes()),
    };
    decode_response(out)?;
    Ok(out.len())
}
pub fn decode_response(bytes: &[u8]) -> Result<(Envelope, Response), Refusal> {
    let (env, op, b) = header(bytes)?;
    let r = match op {
        0x8001 => {
            exact(b, 4)?;
            let revision = u32_at(b, 0)?;
            if revision == 0 {
                return Err(Refusal::Malformed);
            }
            Response::Ready { revision }
        }
        0x8002 => {
            exact(b, 9)?;
            let revision = u32_at(b, 1)?;
            let bytes = u32_at(b, 5)?;
            if revision == 0 || bytes == 0 || bytes > MAX_BLOB_BYTES {
                return Err(Refusal::Bounds);
            }
            Response::Blob {
                kind: output(b[0])?,
                revision,
                bytes,
            }
        }
        0x8003 => {
            exact(b, 13)?;
            let offset = u32_at(b, 0)?;
            let bytes = u32_at(b, 4)?;
            let total = u32_at(b, 8)?;
            if bytes == 0
                || bytes > TRANSFER_BYTES
                || total > MAX_BLOB_BYTES
                || offset.checked_add(bytes).is_none_or(|n| n > total)
                || b[12] > 1
                || ((b[12] == 1) != (offset + bytes == total))
            {
                return Err(Refusal::Bounds);
            }
            Response::Chunk {
                offset,
                bytes,
                total,
                complete: b[12] == 1,
            }
        }
        0xffff => {
            exact(b, 2)?;
            Response::Refused(refusal(u16_at(b, 0)?)?)
        }
        _ => return Err(Refusal::Malformed),
    };
    Ok((env, r))
}
