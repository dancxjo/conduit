//! Canonical bounded transport of an admitted general report.
//! Domain codecs preserve complete values or exact references to separately
//! retained artifacts. Verification requires a reconstructed, admitted report;
//! arbitrary bytes and their digest do not establish semantic validity.
use super::*;
mod encode;
mod writer;
pub use writer::ProjectionEvidenceWriter;

pub const PROJECTION_EVIDENCE_SCHEMA: &str = "conduit/projection-evidence@1";
pub const MAX_PROJECTION_EVIDENCE_BYTES: usize = 1024 * 1024;
const MAGIC: &[u8; 4] = b"CPE\x01";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionEvidenceRefusal {
    InvalidLimits,
    Extent,
    BufferTooSmall,
    EvidenceMismatch,
    UnsupportedVersion,
    PolicyConfigurationUnavailable,
    PolicyConfigurationMismatch,
    DomainEncoding,
}
#[derive(Debug, Clone, Copy)]
pub struct ProjectionEvidenceLimits {
    pub source_bytes: usize,
    pub target_bytes: usize,
    pub detail_bytes: usize,
    pub policy_bytes: usize,
    pub total_bytes: usize,
}
impl ProjectionEvidenceLimits {
    fn validate(self) -> Result<(), ProjectionEvidenceRefusal> {
        if self.total_bytes == 0
            || self.total_bytes > MAX_PROJECTION_EVIDENCE_BYTES
            || self.source_bytes > 262144
            || self.target_bytes > 262144
            || self.detail_bytes > MAX_PROJECTION_NATIVE_BYTES
            || self.policy_bytes > 65536
        {
            return Err(ProjectionEvidenceRefusal::InvalidLimits);
        }
        Ok(())
    }
}
/// Reviewed pure, bounded, allocation-free domain encoding. The identity fixes
/// source, target, typed detail and configuration encodings. Exact external
/// artifact references require those artifacts for independent reconstruction.
/// The configuration digest MUST identify the exact configuration emitted here.
/// Each field callback runs once for measurement and three times for encoding
/// or verification; account for that finite work during preparation.
pub trait ProjectionEvidenceCodec<D: ProjectionDomain> {
    fn identity(&self) -> ProjectionText<'_>;
    fn configuration_digest(&self) -> [u8; 32];
    fn source(
        &self,
        value: &D::Source,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal>;
    fn target(
        &self,
        value: &D::Target,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal>;
    fn detail(
        &self,
        value: &D::Detail,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal>;
    fn policy(
        &self,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal>;
}

pub fn projection_evidence_len<D: ProjectionDomain, C: ProjectionEvidenceCodec<D>>(
    report: &ProjectionReport<'_, D>,
    codec: &C,
    limits: ProjectionEvidenceLimits,
) -> Result<usize, ProjectionEvidenceRefusal> {
    limits.validate()?;
    let digest = report
        .policy_configuration_digest()
        .ok_or(ProjectionEvidenceRefusal::PolicyConfigurationUnavailable)?;
    if digest != codec.configuration_digest() {
        return Err(ProjectionEvidenceRefusal::PolicyConfigurationMismatch);
    }
    let mut out = ProjectionEvidenceWriter::measuring(limits.total_bytes);
    encode::report(report, codec, limits, &mut out)?;
    Ok(out.len())
}
/// Writes only into caller-owned staged storage, with no allocation or growth.
/// A short buffer is refused before writing. A failing/nondeterministic domain
/// codec never yields committed evidence; callers commit only the returned span.
pub fn encode_projection_evidence<D: ProjectionDomain, C: ProjectionEvidenceCodec<D>>(
    report: &ProjectionReport<'_, D>,
    codec: &C,
    limits: ProjectionEvidenceLimits,
    output: &mut [u8],
) -> Result<usize, ProjectionEvidenceRefusal> {
    let length = projection_evidence_len(report, codec, limits)?;
    if output.len() < length {
        return Err(ProjectionEvidenceRefusal::BufferTooSmall);
    }
    let mut out = ProjectionEvidenceWriter::writing(&mut output[..length]);
    encode::report(report, codec, limits, &mut out)?;
    if out.len() != length {
        return Err(ProjectionEvidenceRefusal::DomainEncoding);
    }
    Ok(length)
}
/// Compare every canonical byte against an independently reconstructed admitted
/// report, including complete inventory, original scores, attempts and policy.
/// Rejects unknown versions, omissions, altered fields and trailing bytes.
/// Faithfully transporting an insufficient/refused report does not authorize
/// consuming its target; require_exact/require_bound_policy remain separate.
pub fn verify_projection_evidence<D: ProjectionDomain, C: ProjectionEvidenceCodec<D>>(
    report: &ProjectionReport<'_, D>,
    codec: &C,
    limits: ProjectionEvidenceLimits,
    bytes: &[u8],
) -> Result<(), ProjectionEvidenceRefusal> {
    if bytes.get(..4) != Some(MAGIC.as_slice()) {
        return Err(ProjectionEvidenceRefusal::UnsupportedVersion);
    }
    let length = projection_evidence_len(report, codec, limits)?;
    if length != bytes.len() {
        return Err(ProjectionEvidenceRefusal::EvidenceMismatch);
    }
    let mut out = ProjectionEvidenceWriter::comparing(bytes);
    encode::report(report, codec, limits, &mut out)?;
    if out.len() != bytes.len() {
        return Err(ProjectionEvidenceRefusal::EvidenceMismatch);
    }
    Ok(())
}
/// Content correlation only; not authority, a signature, or execution evidence.
pub fn projection_evidence_digest(bytes: &[u8]) -> Result<[u8; 32], ProjectionEvidenceRefusal> {
    if bytes.len() > MAX_PROJECTION_EVIDENCE_BYTES || bytes.get(..4) != Some(MAGIC.as_slice()) {
        return Err(ProjectionEvidenceRefusal::Extent);
    }
    Ok(crate::semantic_digest(PROJECTION_EVIDENCE_SCHEMA, bytes))
}
