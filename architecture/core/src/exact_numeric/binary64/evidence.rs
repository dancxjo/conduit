//! Complete ordered-computation receipts in the shared projection transport.
//! Source and physical custody are supplied separately by the calling boundary.
use super::*;
use crate::{projection::*, EXACT_RATIONAL_MAX_ENCODED_BYTES};
use sha2::{Digest, Sha256};
type E = ProjectionEvidenceRefusal;
type W<'a> = ProjectionEvidenceWriter<'a>;
const RATIONAL: usize = 4 + EXACT_RATIONAL_MAX_ENCODED_BYTES;
/// Every step carries at most seven rationals, operation references, bits,
/// rounding/status and actual/admitted work. The profile has at most 32 steps.
pub const BINARY64_ALGORITHM_EVIDENCE_LIMITS: ProjectionEvidenceLimits = ProjectionEvidenceLimits {
    source_bytes: BINARY64_ALGORITHM_MAX_STEPS * (7 * RATIONAL + 96) + 4 * RATIONAL + 128,
    target_bytes: 8,
    detail_bytes: 1 + RATIONAL,
    policy_bytes: 1 + RATIONAL,
    total_bytes: 262144,
};
fn mode(value: Binary64Rounding) -> u8 {
    match value {
        Binary64Rounding::NearestTiesEven => 0,
        Binary64Rounding::TowardZero => 1,
        Binary64Rounding::TowardPositive => 2,
        Binary64Rounding::TowardNegative => 3,
    }
}
pub(super) fn policy_digest(policy: Binary64AlgorithmPolicy) -> [u8; 32] {
    let mut bytes = [0; EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let length = policy
        .maximum_error
        .encode(&mut bytes)
        .expect("bounded rational");
    let mut hash = Sha256::new();
    hash.update(b"numeric/finite-binary64-policy@1");
    hash.update([mode(policy.rounding)]);
    hash.update((length as u32).to_le_bytes());
    hash.update(&bytes[..length]);
    hash.finalize().into()
}
fn rational(value: ExactRational, out: &mut W<'_>) -> Result<(), E> {
    let mut bytes = [0; EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let length = value.encode(&mut bytes).map_err(|_| E::DomainEncoding)?;
    out.bytes(&bytes[..length])
}
fn error(value: Binary64ErrorClaim, out: &mut W<'_>) -> Result<(), E> {
    out.tag(match value {
        Binary64ErrorClaim::ExactDistance(_) => 0,
        Binary64ErrorClaim::CertifiedBound(_) => 1,
    })?;
    rational(value.maximum(), out)
}
fn operation(value: &Binary64Operation, out: &mut W<'_>) -> Result<(), E> {
    use Binary64Operation as O;
    match value {
        O::Constant(q) => {
            out.tag(0)?;
            rational(**q, out)
        }
        O::IeeeConstant(bits) => {
            out.tag(1)?;
            out.u64(bits.bits())
        }
        O::ProjectedConstant(input) => {
            out.tag(7)?;
            rational(input.source(), out)?;
            out.u64(input.target().bits())?;
            rational(input.maximum_error(), out)
        }
        O::Negate { input } => {
            out.tag(6)?;
            out.tag(*input)
        }
        O::Add { left, right }
        | O::Subtract { left, right }
        | O::Multiply { left, right }
        | O::Divide { left, right } => {
            out.tag(match value {
                O::Add { .. } => 2,
                O::Subtract { .. } => 3,
                O::Multiply { .. } => 4,
                _ => 5,
            })?;
            out.tag(*left)?;
            out.tag(*right)
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Binary64AlgorithmEvidenceCodec {
    policy: Binary64AlgorithmPolicy,
}
impl Binary64AlgorithmEvidenceCodec {
    pub const fn new(policy: Binary64AlgorithmPolicy) -> Self {
        Self { policy }
    }
}
impl ProjectionEvidenceCodec<Binary64AlgorithmDomain> for Binary64AlgorithmEvidenceCodec {
    fn identity(&self) -> ProjectionText<'_> {
        ProjectionText::new("numeric/finite-binary64-computation-evidence@1").unwrap()
    }
    fn configuration_digest(&self) -> [u8; 32] {
        policy_digest(self.policy)
    }
    fn source(&self, value: &PreparedBinary64Algorithm, out: &mut W<'_>) -> Result<(), E> {
        out.tag(1)?;
        out.tag(value.limits().maximum_steps)?;
        out.u32(value.limits().maximum_retained_bytes)?;
        out.tag(mode(value.rounding()))?;
        rational(value.maximum_error(), out)?;
        out.u64(value.admitted_limb_work())?;
        out.u64(value.spent_limb_work())?;
        out.u64(value.retained_payload_bytes() as u64)?;
        out.tag(value.steps().len() as u8)?;
        for step in value.steps() {
            operation(step.operation(), out)?;
            rational(step.semantic_value(), out)?;
            let p = step.projection();
            rational(p.source(), out)?;
            out.u64(p.target().bits())?;
            rational(p.represented(), out)?;
            out.tag(mode(p.rounding()))?;
            out.tag(u8::from(p.status().inexact))?;
            out.tag(u8::from(p.status().tiny_inexact))?;
            error(p.error(), out)?;
            rational(p.maximum_error(), out)?;
            out.u64(p.admitted_limb_work())?;
            out.u64(p.spent_limb_work())?;
        }
        rational(value.source(), out)?;
        out.u64(value.target().bits())?;
        error(value.error(), out)
    }
    fn target(&self, value: &IeeeF64, out: &mut W<'_>) -> Result<(), E> {
        out.u64(value.bits())
    }
    fn detail(&self, value: &Binary64ErrorClaim, out: &mut W<'_>) -> Result<(), E> {
        error(*value, out)
    }
    fn policy(&self, out: &mut W<'_>) -> Result<(), E> {
        out.tag(mode(self.policy.rounding))?;
        rational(self.policy.maximum_error, out)
    }
}
