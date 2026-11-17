//! Borrow the existing owner transaction payload until its journal is durable.
//! The owned decoding/recovery representation remains the schema authority.
use super::*;

/// Serialize in exactly the owned Transaction's field order, including nulls.
/// This view creates no copies of biography, receipts, Sources or archives.
#[derive(Serialize)]
pub(super) struct View<'a> {
    pub(super) schema: &'a str,
    pub(super) biography: &'a BodyBiographyEvidence,
    pub(super) installation: &'a Installation,
    pub(super) last_execution: Option<&'a serde_json::Value>,
    pub(super) execution_custody: Option<&'a execution_custody::Prepared>,
    pub(super) admissions: Option<&'a conduit_body::AdmissionManager>,
    pub(super) admission_custody: Option<&'a admission_custody::Prepared>,
    pub(super) source: Option<&'a [u8]>,
    pub(super) workset_sources: Option<&'a WorksetSources>,
    pub(super) archives: &'a [BodyBiographyArchiveSegment],
}

#[cfg(test)]
#[path = "state_transaction/tests.rs"]
mod tests;
