//! A finite duplex attachment handshake. Ordinary control requests remain
//! EOF-delimited; this distinct magic allows an acknowledgement on the same
//! authenticated Unix connection after the initial request.

use conduit_body::BodyId;
use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration, PlanId};
use conduit_presentation::{MaskShow, PresentationContentId};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

pub(super) const MAGIC: &[u8; 8] = b"CDTERM01";
const MAX_HANDSHAKE_BYTES: usize = 4_096;
const MAX_REPLY_BYTES: usize = 512 * 1024;
const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(2);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AttachRequest {
    pub protocol: u16,
    pub token: Vec<u8>,
    pub body_id: BodyId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub face_id: PresentationContentId,
    pub face_revision: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum AttachReply {
    Attached { protocol: u16 },
    Show {
        protocol: u16,
        route_plan_id: PlanId,
        show: Box<MaskShow>,
        advertisement: HostAdvertisement,
    },
    Refused { protocol: u16, code: String },
}

pub(super) fn read_request(stream: &mut UnixStream, first: u8) -> Result<AttachRequest, String> {
    let deadline = Instant::now() + HANDSHAKE_DEADLINE;
    let mut magic = [0; 8];
    magic[0] = first;
    read_exact_until(stream, &mut magic[1..], deadline)?;
    if &magic != MAGIC {
        return Err("terminal-attachment-protocol".into());
    }
    read_json(stream, MAX_HANDSHAKE_BYTES, deadline)
}

pub(super) fn write_request(stream: &mut UnixStream, request: &AttachRequest) -> Result<(), String> {
    let deadline = Instant::now() + HANDSHAKE_DEADLINE;
    write_all_until(stream, MAGIC, deadline)?;
    write_json(stream, request, MAX_HANDSHAKE_BYTES, deadline)
}

pub(super) fn read_reply(stream: &mut UnixStream) -> Result<AttachReply, String> {
    read_json(stream, MAX_REPLY_BYTES, Instant::now() + HANDSHAKE_DEADLINE)
}

pub(super) fn write_reply(stream: &mut UnixStream, reply: &AttachReply) -> Result<(), String> {
    write_json(
        stream,
        reply,
        MAX_REPLY_BYTES,
        Instant::now() + HANDSHAKE_DEADLINE,
    )
}

fn read_json<T: DeserializeOwned>(
    stream: &mut UnixStream,
    limit: usize,
    deadline: Instant,
) -> Result<T, String> {
    let mut length = [0; 4];
    read_exact_until(stream, &mut length, deadline)?;
    let length = usize::try_from(u32::from_le_bytes(length))
        .map_err(|_| "terminal-attachment-frame-pressure".to_string())?;
    if length == 0 || length > limit {
        return Err("terminal-attachment-frame-pressure".into());
    }
    let mut bytes = vec![0; length];
    read_exact_until(stream, &mut bytes, deadline)?;
    let result = serde_json::from_slice(&bytes)
        .map_err(|_| "terminal-attachment-malformed-frame".to_string());
    bytes.fill(0);
    result
}

fn write_json<T: Serialize>(
    stream: &mut UnixStream,
    value: &T,
    limit: usize,
    deadline: Instant,
) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|_| "terminal-attachment-frame-encoding".to_string())?;
    if bytes.is_empty() || bytes.len() > limit {
        bytes.fill(0);
        return Err("terminal-attachment-frame-pressure".into());
    }
    let length = u32::try_from(bytes.len())
        .map_err(|_| "terminal-attachment-frame-pressure".to_string())?;
    let result = write_all_until(stream, &length.to_le_bytes(), deadline)
        .and_then(|()| write_all_until(stream, &bytes, deadline));
    bytes.fill(0);
    result
}

fn read_exact_until(stream: &mut UnixStream, mut bytes: &mut [u8], deadline: Instant) -> Result<(), String> {
    while !bytes.is_empty() {
        stream
            .set_read_timeout(Some(remaining(deadline)?))
            .map_err(|_| "terminal-attachment-I/O".to_string())?;
        match stream.read(bytes) {
            Ok(0) => return Err("terminal-attachment-EOF".into()),
            Ok(count) => bytes = &mut bytes[count..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut || error.kind() == std::io::ErrorKind::WouldBlock => {
                return Err("terminal-attachment-timeout".into());
            }
            Err(_) => return Err("terminal-attachment-I/O".into()),
        }
    }
    Ok(())
}

fn write_all_until(stream: &mut UnixStream, mut bytes: &[u8], deadline: Instant) -> Result<(), String> {
    while !bytes.is_empty() {
        stream
            .set_write_timeout(Some(remaining(deadline)?))
            .map_err(|_| "terminal-attachment-I/O".to_string())?;
        match stream.write(bytes) {
            Ok(0) => return Err("terminal-attachment-EOF".into()),
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut || error.kind() == std::io::ErrorKind::WouldBlock => {
                return Err("terminal-attachment-timeout".into());
            }
            Err(_) => return Err("terminal-attachment-I/O".into()),
        }
    }
    Ok(())
}

fn remaining(deadline: Instant) -> Result<Duration, String> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| "terminal-attachment-timeout".into())
}
