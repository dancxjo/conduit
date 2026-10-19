//! Complete domain fields for the shared general projection evidence format.
//! This codec transports a computational receipt. Independent verification must
//! replay the source with the same admitted work/profile before comparing it;
//! neither the bytes nor their digest attest producer execution.
use super::*;
use crate::projection::*;
type E = ProjectionEvidenceRefusal;
type Writer<'a> = ProjectionEvidenceWriter<'a>;

// Each rational and Unit is length-prefixed; the full physical source frame
// already has its independent bounded encoding. Source contains thirteen
// rationals, four additional physical frames, four u32 profile limits, two u64
// enclosure counters, one u32 term count, two target bit patterns, rounding and
// inexact tags, two error tags, and two u64 midpoint counters.
const RATIONAL_FIELD: usize = 4 + EXACT_RATIONAL_MAX_ENCODED_BYTES;
const PHYSICAL_FIELD: usize = 4 + UNIT_ENCODED_LEN + 1;
pub const MATHEMATICAL_BINARY32_EVIDENCE_SOURCE_MAX_BYTES: usize = 4
    + MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES
    + 13 * RATIONAL_FIELD
    + 4 * PHYSICAL_FIELD
    + 4 * 4
    + 2 * 8
    + 4
    + 2 * 4
    + 1
    + 1
    + 2
    + 2 * 8;
pub const MATHEMATICAL_BINARY32_EVIDENCE_TARGET_MAX_BYTES: usize = 4 + PHYSICAL_FIELD;
pub const MATHEMATICAL_BINARY32_EVIDENCE_DETAIL_MAX_BYTES: usize = 1 + RATIONAL_FIELD;
pub const MATHEMATICAL_BINARY32_EVIDENCE_POLICY_MAX_BYTES: usize =
    RATIONAL_FIELD + PHYSICAL_FIELD + 1;

/// The exact policy is copied before transport; no mutable ambient configuration.
#[derive(Debug, Clone, Copy)]
pub struct MathematicalQuantityBinary32EvidenceCodec {
    policy: MathematicalQuantityBinary32Policy,
    digest: [u8; 32],
}
impl MathematicalQuantityBinary32EvidenceCodec {
    pub fn new(policy: MathematicalQuantityBinary32Policy) -> Self {
        let digest = policy
            .configuration_digest()
            .expect("bound mathematical policy");
        Self { policy, digest }
    }
}
impl<'a> ProjectionEvidenceCodec<MathematicalQuantityBinary32Domain<'a>>
    for MathematicalQuantityBinary32EvidenceCodec
{
    fn identity(&self) -> ProjectionText<'_> {
        ProjectionText::new("numeric/enclosed-quantity-binary32-evidence@1").unwrap()
    }
    fn configuration_digest(&self) -> [u8; 32] {
        self.digest
    }
    fn source(
        &self,
        value: &BorrowedMathematicalQuantityBinary32<'a>,
        out: &mut Writer<'_>,
    ) -> Result<(), E> {
        // Full physical symbolic frame, rather than only its representation hash.
        out.bytes(value.source().encoded())?;
        let enclosure = value.enclosure().numerical();
        rational(enclosure.target().lower(), out)?;
        rational(enclosure.target().upper(), out)?;
        rational(enclosure.width(), out)?;
        let profile = enclosure.requirements();
        rational(profile.maximum_width, out)?;
        rational(profile.maximum_leaf_width, out)?;
        out.u32(u32::from(profile.maximum_constant_terms))?;
        out.u32(profile.maximum_root_refinements)?;
        out.u32(profile.maximum_scratch_payload_bytes)?;
        out.u32(profile.maximum_retained_payload_bytes)?;
        out.u64(enclosure.admitted_limb_work())?;
        out.u64(enclosure.limb_work())?;
        out.u32(u32::from(enclosure.maximum_used_constant_terms()))?;
        let midpoint = value.midpoint();
        quantity(midpoint.source(), out)?;
        target(midpoint.target(), out)?;
        quantity(midpoint.maximum_error(), out)?;
        let numeric = midpoint.numeric();
        rational(numeric.source(), out)?;
        out.u32(numeric.target().bits())?;
        rational(numeric.represented(), out)?;
        rounding(numeric.rounding(), out)?;
        out.tag(u8::from(numeric.inexact()))?;
        error(numeric.error(), out)?;
        rational(numeric.maximum_error(), out)?;
        out.u64(numeric.spent_limb_work())?;
        out.u64(numeric.admitted_limb_work())?;
        error(value.error(), out)?;
        quantity(value.maximum_error(), out)
    }
    fn target(&self, value: &QuantityBinary32, out: &mut Writer<'_>) -> Result<(), E> {
        target(*value, out)
    }
    fn detail(&self, value: &Binary32ErrorClaim, out: &mut Writer<'_>) -> Result<(), E> {
        error(*value, out)
    }
    fn policy(&self, out: &mut Writer<'_>) -> Result<(), E> {
        quantity(self.policy.maximum_error, out)?;
        rounding(self.policy.rounding, out)
    }
}
fn rational(value: ExactRational, out: &mut Writer<'_>) -> Result<(), E> {
    let mut bytes = [0; EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let length = value.encode(&mut bytes).map_err(|_| E::DomainEncoding)?;
    out.bytes(&bytes[..length])
}
fn physical(unit: Unit, role: QuantityRole, out: &mut Writer<'_>) -> Result<(), E> {
    out.bytes(&unit.encode())?;
    out.tag(match role {
        QuantityRole::Linear => 0,
        QuantityRole::Point => 1,
        QuantityRole::Delta => 2,
    })
}
fn quantity(value: RationalQuantity, out: &mut Writer<'_>) -> Result<(), E> {
    rational(value.coordinate(), out)?;
    physical(value.unit(), value.role(), out)
}
fn target(value: QuantityBinary32, out: &mut Writer<'_>) -> Result<(), E> {
    out.u32(value.coordinate().bits())?;
    physical(value.unit(), value.role(), out)
}
fn rounding(value: Binary32Rounding, out: &mut Writer<'_>) -> Result<(), E> {
    out.tag(match value {
        Binary32Rounding::NearestTiesEven => 0,
        Binary32Rounding::TowardZero => 1,
        Binary32Rounding::TowardPositive => 2,
        Binary32Rounding::TowardNegative => 3,
    })
}
fn error(value: Binary32ErrorClaim, out: &mut Writer<'_>) -> Result<(), E> {
    let (tag, amount) = match value {
        Binary32ErrorClaim::ExactDistance(q) => (0, q),
        Binary32ErrorClaim::CertifiedBound(q) => (1, q),
    };
    out.tag(tag)?;
    rational(amount, out)
}
