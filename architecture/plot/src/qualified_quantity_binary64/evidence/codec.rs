//! Shared CPE codec with explicit digest/length references to retained artifacts.
use super::*;
type E = ProjectionEvidenceRefusal;
type W<'a> = ProjectionEvidenceWriter<'a>;
pub const QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS: ProjectionEvidenceLimits =
    ProjectionEvidenceLimits {
        source_bytes: BINARY64_ALGORITHM_EVIDENCE_LIMITS.source_bytes
            + 512
            + EXACT_RATIONAL_MAX_ENCODED_BYTES
            + UNIT_ENCODED_LEN,
        target_bytes: 16 + UNIT_ENCODED_LEN,
        detail_bytes: BINARY64_ALGORITHM_EVIDENCE_LIMITS.detail_bytes,
        policy_bytes: 128 + UNIT_ENCODED_LEN + BINARY64_ALGORITHM_EVIDENCE_LIMITS.policy_bytes,
        total_bytes: 262144,
    };
#[derive(Debug, Clone, Copy)]
pub struct QualifiedQuantityBinary64EvidenceCodec {
    policy: QualifiedQuantityBinary64Policy,
}
impl QualifiedQuantityBinary64EvidenceCodec {
    pub const fn new(policy: QualifiedQuantityBinary64Policy) -> Self {
        Self { policy }
    }
}
impl<'a> ProjectionEvidenceCodec<QualifiedQuantityBinary64Domain<'a>>
    for QualifiedQuantityBinary64EvidenceCodec
{
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/source-bound-binary64-evidence@1")
    }
    fn configuration_digest(&self) -> [u8; 32] {
        self.policy.digest
    }
    fn source(
        &self,
        value: &QualifiedQuantityBinary64Result<'a>,
        out: &mut W<'_>,
    ) -> Result<(), E> {
        out.tag(1)?;
        // These are exact external artifact references, not Source/Plan/Play identities.
        out.raw(&value.policy.program_digest)?;
        out.u64(value.canonical_program.len() as u64)?;
        out.raw(&value.incoming_digest)?;
        out.u64(value.incoming_evidence.len() as u64)?;
        out.raw(&value.incoming_source_digest)?;
        let mut bytes = [0; EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let n = value
            .source
            .coordinate()
            .encode(&mut bytes)
            .map_err(|_| E::DomainEncoding)?;
        out.bytes(&bytes[..n])?;
        out.bytes(&value.source.unit().encode())?;
        out.tag(role_tag(value.source.role()))?;
        Binary64AlgorithmEvidenceCodec::new(self.policy.numeric).source(value.algorithm, out)
    }
    fn target(&self, value: &QualifiedQuantityBinary64Target, out: &mut W<'_>) -> Result<(), E> {
        out.u64(value.coordinate.bits())?;
        out.bytes(&value.unit.encode())?;
        out.tag(role_tag(value.role))
    }
    fn detail(&self, value: &Binary64ErrorClaim, out: &mut W<'_>) -> Result<(), E> {
        Binary64AlgorithmEvidenceCodec::new(self.policy.numeric).detail(value, out)
    }
    fn policy(&self, out: &mut W<'_>) -> Result<(), E> {
        out.raw(&self.policy.program_digest)?;
        out.bytes(&self.policy.unit.encode())?;
        out.tag(role_tag(self.policy.role))?;
        Binary64AlgorithmEvidenceCodec::new(self.policy.numeric).policy(out)
    }
}
