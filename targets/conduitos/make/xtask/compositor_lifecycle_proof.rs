//! Pure terminal receipt validation. No I/O, guest calls, clock or artifact custody.
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
        let record: types::Record = serde_json::from_str(line.strip_prefix(PREFIX).unwrap())
            .map_err(|_| Refusal::Malformed)?;
        if record.schema != "conduit.conduitos/compositor-lifecycle@1" {
            return Err(Refusal::UnknownSchema);
        }
        if !record.complete || record.record_kind != types::RecordKind::Snapshot {
            return Err(Refusal::Incomplete);
        }
        records.push(record.snapshot);
    }
    validate::validate(&records, &frames, expected)
}
