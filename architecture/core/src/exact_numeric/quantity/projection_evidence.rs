//! Full exact/single-round physical receipts in the shared general report format.
//! Original wire provenance is the report basis and separately retained input;
//! the domain payload contains complete typed denotation and computation fields.
use super::evidence_fields::*;
use super::*;
use crate::projection::*;
type E = ProjectionEvidenceRefusal;
type W<'a> = ProjectionEvidenceWriter<'a>;
const Q: usize = RATIONAL_FIELD + PHYSICAL_FIELD;
const T: usize = 4 + PHYSICAL_FIELD;
pub const QUANTITY_BINARY32_EVIDENCE_SOURCE_MAX_BYTES: usize =
    2 * Q + 4 * RATIONAL_FIELD + T + 4 + 3 + 16;
pub const QUANTITY_BINARY32_EVIDENCE_TARGET_MAX_BYTES: usize = T;
pub const QUANTITY_BINARY32_EVIDENCE_DETAIL_MAX_BYTES: usize = 1 + RATIONAL_FIELD;
pub const QUANTITY_BINARY32_EVIDENCE_POLICY_MAX_BYTES: usize = Q + 1;
/// Fixed Near inventory: three facts, bounded content identities, no attempts or
/// native metrics. Other report inventories must admit their own total ceiling.
pub const RATIONAL_NEAR_PROJECTION_EVIDENCE_MAX_BYTES: usize = 16384;
pub const RATIONAL_NEAR_PROJECTION_EVIDENCE_LIMITS: ProjectionEvidenceLimits =
    ProjectionEvidenceLimits {
        source_bytes: QUANTITY_BINARY32_EVIDENCE_SOURCE_MAX_BYTES,
        target_bytes: Q,
        detail_bytes: QUANTITY_BINARY32_EVIDENCE_DETAIL_MAX_BYTES,
        policy_bytes: QUANTITY_BINARY32_EVIDENCE_POLICY_MAX_BYTES,
        total_bytes: RATIONAL_NEAR_PROJECTION_EVIDENCE_MAX_BYTES,
    };
#[derive(Debug, Clone, Copy)]
pub struct QuantityBinary32EvidenceCodec {
    policy: QuantityBinary32ProjectionPolicy,
    digest: [u8; 32],
}
impl QuantityBinary32EvidenceCodec {
    pub fn new(policy: QuantityBinary32ProjectionPolicy) -> Self {
        let digest = policy
            .configuration_digest()
            .expect("bound physical binary32 policy");
        Self { policy, digest }
    }
}
impl ProjectionEvidenceCodec<QuantityBinary32ProjectionDomain> for QuantityBinary32EvidenceCodec {
    fn identity(&self) -> ProjectionText<'_> {
        ProjectionText::new("numeric/quantity-binary32-evidence@1").unwrap()
    }
    fn configuration_digest(&self) -> [u8; 32] {
        self.digest
    }
    fn source(&self, value: &PreparedQuantityBinary32Projection, out: &mut W<'_>) -> Result<(), E> {
        quantity(value.source(), out)?;
        let n = value.numeric();
        rational(n.source(), out)?;
        out.u32(n.target().bits())?;
        rational(n.represented(), out)?;
        rounding(n.rounding(), out)?;
        out.tag(u8::from(n.inexact()))?;
        error(n.error(), out)?;
        rational(n.maximum_error(), out)?;
        out.u64(n.spent_limb_work())?;
        out.u64(n.admitted_limb_work())?;
        target(value.target(), out)?;
        quantity(value.maximum_error(), out)
    }
    fn target(&self, value: &QuantityBinary32, out: &mut W<'_>) -> Result<(), E> {
        target(*value, out)
    }
    fn detail(&self, value: &crate::Binary32ErrorClaim, out: &mut W<'_>) -> Result<(), E> {
        error(*value, out)
    }
    fn policy(&self, out: &mut W<'_>) -> Result<(), E> {
        quantity(self.policy.maximum_error, out)?;
        rounding(self.policy.rounding, out)
    }
}
#[derive(Debug, Clone, Copy)]
pub struct ExactNearEvidenceCodec {
    policy: ExactNearProjectionPolicy,
    digest: [u8; 32],
}
impl ExactNearEvidenceCodec {
    pub fn new(policy: ExactNearProjectionPolicy) -> Self {
        let digest = policy
            .configuration_digest()
            .expect("bound exact physical policy");
        Self { policy, digest }
    }
}
impl ProjectionEvidenceCodec<ExactNearProjectionDomain> for ExactNearEvidenceCodec {
    fn identity(&self) -> ProjectionText<'_> {
        ProjectionText::new("numeric/near-exact-rational-evidence@1").unwrap()
    }
    fn configuration_digest(&self) -> [u8; 32] {
        self.digest
    }
    fn source(&self, value: &RationalQuantity, out: &mut W<'_>) -> Result<(), E> {
        quantity(*value, out)
    }
    fn target(&self, value: &RationalQuantity, out: &mut W<'_>) -> Result<(), E> {
        quantity(*value, out)
    }
    fn detail(&self, _: &(), out: &mut W<'_>) -> Result<(), E> {
        out.raw(&[])
    }
    fn policy(&self, out: &mut W<'_>) -> Result<(), E> {
        quantity(self.policy.maximum_error, out)
    }
}
