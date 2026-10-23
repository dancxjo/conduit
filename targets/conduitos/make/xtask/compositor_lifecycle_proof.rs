//! Pure terminal receipt validation. No I/O, guest calls, clock or artifact custody.
extern crate std;
use self::std::vec::Vec;
#[path = "compositor_lifecycle_proof/frame.rs"]
mod frame;
#[path = "compositor_lifecycle_proof/types.rs"]
mod types;
#[path = "compositor_lifecycle_proof/validate.rs"]
mod validate;
pub use types::*;

pub const PREFIX: &str = "CONDUIT_COMPOSITOR_LIFECYCLE ";
pub const MAX_LINE_BYTES: usize = 4096;
pub const MAX_RECORDS: usize = 32;
const MAX_FRAMES: usize = 4096;

/// Input is terminal, already decoded by the owning shared serial reader.
/// Unrelated lines are ignored; a partial required record always refuses.
pub fn parse_terminal(serial: &str, expected: &ExpectedBinding) -> Result<Report, Refusal> {
    let (records, frames) = decode_terminal(serial)?;
    validate::validate(&records, &frames, expected)
}

/// Observed preparation metadata only; no artifact or full-scope claim follows.
/// Reuses the exact closed decoder, including incomplete-marker refusal.
pub fn preparation_context(serial: &str) -> Result<Snapshot, Refusal> {
    let (records, _) = decode_terminal(serial)?;
    let mut contexts = records.into_iter().filter(|r| {
        r.checkpoint == Checkpoint::Prepared && r.kind == Kind::Context && r.position.part == 0
    });
    let context = contexts.next().ok_or(Refusal::BatchShape)?;
    if contexts.next().is_some() {
        return Err(Refusal::BatchShape);
    }
    Ok(context)
}

fn decode_terminal(serial: &str) -> Result<(Vec<Snapshot>, Vec<frame::Frame>), Refusal> {
    let mut records = Vec::new();
    let mut frames = Vec::new();
    for line in serial.split_inclusive('\n') {
        let lifecycle = line.starts_with(PREFIX);
        let reserved =
            line.starts_with(PREFIX.trim_end()) || line.starts_with(frame::PREFIX.trim_end());
        if reserved && !line.ends_with('\n') {
            return Err(Refusal::Truncated);
        }
        if reserved && !lifecycle && !line.starts_with(frame::PREFIX) {
            return Err(Refusal::Malformed);
        }
        let frame = line.starts_with(frame::PREFIX);
        if !lifecycle && !frame {
            continue;
        }
        if !line.ends_with('\n') {
            return Err(Refusal::Truncated);
        }
        if line.len() > MAX_LINE_BYTES {
            return Err(Refusal::Oversize);
        }
        let line = line.strip_suffix('\n').unwrap();
        if frame {
            if frames.len() == MAX_FRAMES {
                return Err(Refusal::RecordCount);
            }
            frames.push(frame::parse(line, records.len())?);
            continue;
        }
        if records.len() == MAX_RECORDS {
            return Err(Refusal::RecordCount);
        }
        let packet: types::Packet = serde_json::from_str(line.strip_prefix(PREFIX).unwrap())
            .map_err(|_| Refusal::Malformed)?;
        match packet {
            types::Packet::Snapshot(record) => {
                let _kind = record.record_kind;
                if record.schema != "conduit.conduitos/compositor-lifecycle@1" {
                    return Err(Refusal::UnknownSchema);
                }
                if !record.complete {
                    return Err(Refusal::Malformed);
                }
                records.push(record.snapshot);
            }
            types::Packet::Incomplete(record) => {
                let _details = (record.record_kind, record.position, record.stage);
                if record.schema != "conduit.conduitos/compositor-lifecycle@1" {
                    return Err(Refusal::UnknownSchema);
                }
                if record.complete
                    || record.reason == IncompleteReason::IncompleteMarkerSerialization
                {
                    return Err(Refusal::Malformed);
                }
                return Err(Refusal::Incomplete(record.reason));
            }
            types::Packet::Emergency(record) => {
                let _kind = record.record_kind;
                if record.schema != "conduit.conduitos/compositor-lifecycle@1" {
                    return Err(Refusal::UnknownSchema);
                }
                if record.complete
                    || record.reason != IncompleteReason::IncompleteMarkerSerialization
                {
                    return Err(Refusal::Malformed);
                }
                return Err(Refusal::Incomplete(record.reason));
            }
        }
    }
    Ok((records, frames))
}

/// Join a separate appliance observation to the existing real successful ACK line.
pub(super) fn verify_ack(serial: &str, ack: &AckRef, cost: &Cost) -> Result<(), Refusal> {
    let mut matched = 0;
    let mut count = 0;
    for line in serial
        .lines()
        .filter(|line| line.starts_with(frame::PREFIX))
    {
        count += 1;
        if count > MAX_FRAMES || line.len() > MAX_LINE_BYTES {
            return Err(Refusal::Frame);
        }
        let frame = frame::parse(line, 0)?;
        if &frame.ack == ack {
            if frame.counters
                != [
                    cost.entries,
                    cost.privilege_transitions,
                    cost.address_space_switches,
                    cost.scheduler_returns,
                ]
            {
                return Err(Refusal::Ack);
            }
            matched += 1;
        }
    }
    if matched != 1 {
        return Err(Refusal::Ack);
    }
    Ok(())
}
