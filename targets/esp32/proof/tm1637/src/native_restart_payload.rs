//! Complete private restart envelope; no pristine-body replacement or lost
//! archive segment can satisfy its canonical continuation checks.
use alloc::{string::String, vec::Vec};
use conduit_body::{AdmissionManager, BodyBiographyArchiveSegment, BodyBiographyEvidence};
use serde::{Deserialize, Serialize};
#[path = "native_restart_profile.rs"]
mod profile;
#[path = "native_restart_read_json.rs"]
mod read_json;
pub const SCHEMA: &str = "conduit.environment/root-restart@1";
pub const MAX_BYTES: usize = 32768 - 160;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retained {
    pub schema: String,
    #[serde(deserialize_with = "profile::deserialize_evidence")]
    pub evidence: BodyBiographyEvidence,
    #[serde(deserialize_with = "profile::deserialize_archives")]
    pub archives: Vec<BodyBiographyArchiveSegment>,
    #[serde(default)]
    pub admissions: Option<AdmissionManager>,
}
#[derive(Serialize)]
struct Snapshot<'a> {
    schema: &'static str,
    evidence: profile::EvidenceRef<'a>,
    archives: Archives<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    admissions: Option<&'a AdmissionManager>,
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
            seq.serialize_element(&profile::ArchiveRef::from(segment))?;
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
    profile::ensure_no_holds(evidence, archives)?;
    profile::ensure_no_holds(evidence, pending)?;
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
        evidence: evidence.into(),
        archives: Archives(archives, pending),
        admissions: None,
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
#[cfg(not(target_os = "none"))]
pub fn decode(
    bytes: &[u8],
    expected_body: &conduit_body::BodyId,
) -> Result<Retained, &'static str> {
    if bytes.len() > MAX_BYTES {
        return Err("restart-capacity");
    }
    let retained: Retained = serde_json::from_slice(bytes).map_err(|_| "restart-encoding")?;
    validate_retained(retained, expected_body)
}
/// Identical strict typed DTO and canonical validation, with only bounded input
/// chunks retained. Complete EOF is required before the result can escape.
pub fn decode_streamed(
    read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    expected_body: &conduit_body::BodyId,
) -> Result<Retained, &'static str> {
    let retained = read_json::from_reader(read, MAX_BYTES).map_err(|_| "restart-encoding")?;
    validate_retained(retained, expected_body)
}
fn validate_retained(
    retained: Retained,
    expected_body: &conduit_body::BodyId,
) -> Result<Retained, &'static str> {
    if retained.schema != SCHEMA || &retained.evidence.body_id != expected_body {
        return Err("restart-body-binding");
    }
    validate(&retained.evidence, &retained.archives)?;
    if retained
        .admissions
        .as_ref()
        .is_some_and(|manager| manager.body_id != retained.evidence.body_id)
    {
        return Err("restart-admission-body");
    }
    Ok(retained)
}

#[path = "native_restart_json.rs"]
pub(crate) mod json;
#[derive(Debug)]
pub enum StreamRefusal<E> {
    Sink(E),
    Invalid,
}
/// Serialize the entire canonical envelope without a document allocation.
/// The caller owns bounded staging and must not publish before journal commit.
pub fn stream<E>(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    pending: &[BodyBiographyArchiveSegment],
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), StreamRefusal<E>> {
    stream_with_admissions(evidence, archives, pending, None, sink)
}

pub fn stream_with_admissions<E>(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
    pending: &[BodyBiographyArchiveSegment],
    admissions: Option<&AdmissionManager>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), StreamRefusal<E>> {
    validate_chain(evidence, archives, pending).map_err(|_| StreamRefusal::Invalid)?;
    if admissions.is_some_and(|manager| manager.body_id != evidence.body_id) {
        return Err(StreamRefusal::Invalid);
    }
    let snapshot = Snapshot {
        schema: SCHEMA,
        evidence: evidence.into(),
        archives: Archives(archives, pending),
        admissions,
    };
    let mut failure = None;
    let result = json::stream(&snapshot, &mut |bytes: &[u8]| {
        sink(bytes).map_err(|error| {
            failure = Some(error);
            json::CodecError::Sink
        })
    });
    if let Some(error) = failure {
        return Err(StreamRefusal::Sink(error));
    }
    result.map_err(|_| StreamRefusal::Invalid)
}
