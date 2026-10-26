//! Complete private restart envelope; no pristine-body replacement or lost
//! archive segment can satisfy its canonical continuation checks.
use alloc::{string::String, vec::Vec};
use conduit_body::{BodyBiographyArchiveSegment, BodyBiographyEvidence};
use serde::{Deserialize, Serialize};
pub const SCHEMA: &str = "conduit.environment/root-restart@1";
pub const MAX_BYTES: usize = 32768 - 160;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retained {
    pub schema: String,
    pub evidence: BodyBiographyEvidence,
    pub archives: Vec<BodyBiographyArchiveSegment>,
}
#[derive(Serialize)]
struct Snapshot<'a> {
    schema: &'static str,
    evidence: &'a BodyBiographyEvidence,
    archives: Archives<'a>,
}
struct Archives<'a>(
    &'a [BodyBiographyArchiveSegment],
    &'a [BodyBiographyArchiveSegment],
);
impl Serialize for Archives<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.0.len() + self.1.len()))?;
        for segment in self.0.iter().chain(self.1) {
            seq.serialize_element(segment)?;
        }
        seq.end()
    }
}
pub fn validate(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
) -> Result<(), &'static str> {
    validate_chain(evidence, archives, &[])
}
fn validate_chain(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    pending: &[BodyBiographyArchiveSegment],
) -> Result<(), &'static str> {
    evidence.validate().map_err(|_| "restart-biography")?;
    let mut previous = None;
    let mut ordinal = 0u64;
    for segment in archives.iter().chain(pending) {
        segment.validate().map_err(|_| "restart-archive")?;
        ordinal = ordinal.checked_add(1).ok_or("restart-archive-capacity")?;
        if segment.body_id != evidence.body_id
            || segment.ordinal != ordinal
            || segment.previous_digest != previous
        {
            return Err("restart-archive-chain");
        }
        previous = Some(segment.digest);
    }
    match archives.iter().chain(pending).last() {
        Some(head) => head
            .validate_as_head_of(evidence)
            .map_err(|_| "restart-archive-head")?,
        None if evidence.compaction.as_ref().is_some_and(|summary| {
            summary.sealed_segments != 0 || summary.archive_head_digest.is_some()
        }) =>
        {
            return Err("restart-missing-archive")
        }
        None => (),
    }
    Ok(())
}
pub fn encode(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    pending: &[BodyBiographyArchiveSegment],
) -> Result<Vec<u8>, &'static str> {
    validate_chain(evidence, archives, pending)?;
    let snapshot = Snapshot {
        schema: SCHEMA,
        evidence,
        archives: Archives(archives, pending),
    };
    let mut size = 2048;
    loop {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| "restart-memory")?;
        bytes.resize(size, 0);
        match serde_json_core::to_slice(&snapshot, &mut bytes) {
            Ok(n) => {
                bytes.truncate(n);
                return Ok(bytes);
            }
            Err(serde_json_core::ser::Error::BufferFull) if size < MAX_BYTES => {
                size = (size * 2).min(MAX_BYTES)
            }
            Err(_) => return Err("restart-capacity-or-encoding"),
        }
    }
}
pub fn decode(
    bytes: &[u8],
    expected_body: &conduit_body::BodyId,
) -> Result<Retained, &'static str> {
    if bytes.len() > MAX_BYTES {
        return Err("restart-capacity");
    }
    let retained: Retained = serde_json::from_slice(bytes).map_err(|_| "restart-encoding")?;
    if retained.schema != SCHEMA || &retained.evidence.body_id != expected_body {
        return Err("restart-body-binding");
    }
    validate(&retained.evidence, &retained.archives)?;
    Ok(retained)
}
