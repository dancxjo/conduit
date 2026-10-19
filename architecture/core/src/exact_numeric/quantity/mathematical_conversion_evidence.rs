//! Complete exact coordinate conversion through the shared bounded CPE codec.
//! Decoding framing is not replay: verification needs the actual reconstructed
//! conversion receipt, original local work allowance and selected target Unit.
use super::evidence_fields::{rational, RATIONAL_FIELD};
use super::*;
use crate::projection::*;

pub const MATHEMATICAL_QUANTITY_CONVERSION_EVIDENCE_SOURCE_MAX_BYTES: usize = 4
    + MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES
    + 4
    + crate::UNIT_ENCODED_LEN
    + 2 * RATIONAL_FIELD
    + 4
    + 32
    + 16;
pub const MATHEMATICAL_QUANTITY_CONVERSION_EVIDENCE_LIMITS: ProjectionEvidenceLimits =
    ProjectionEvidenceLimits {
        source_bytes: MATHEMATICAL_QUANTITY_CONVERSION_EVIDENCE_SOURCE_MAX_BYTES,
        target_bytes: 4 + MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES,
        detail_bytes: 0,
        policy_bytes: 4 + crate::UNIT_ENCODED_LEN,
        total_bytes: 131072,
    };

#[derive(Debug, Clone, Copy)]
pub struct MathematicalQuantityConversionEvidenceCodec {
    policy: MathematicalQuantityConversionPolicy,
    digest: [u8; 32],
}
impl MathematicalQuantityConversionEvidenceCodec {
    pub fn new(policy: MathematicalQuantityConversionPolicy) -> Self {
        let digest = <MathematicalQuantityConversionPolicy as ProjectionPolicy<
            MathematicalQuantityConversionDomain<'_, '_>,
        >>::configuration_digest(&policy)
        .expect("bound exact coordinate policy");
        Self { policy, digest }
    }
}
impl<'source, 'target>
    ProjectionEvidenceCodec<MathematicalQuantityConversionDomain<'source, 'target>>
    for MathematicalQuantityConversionEvidenceCodec
{
    fn identity(&self) -> ProjectionText<'_> {
        ProjectionText::new("physical/exact-unit-coordinate-evidence@1").unwrap()
    }
    fn configuration_digest(&self) -> [u8; 32] {
        self.digest
    }
    fn source(
        &self,
        value: &MathematicalQuantityCoordinateConversion<'source>,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.bytes(value.source().encoded())?;
        out.bytes(&value.target_unit().encode())?;
        rational(value.scale(), out)?;
        rational(value.offset(), out)?;
        out.u32(
            u32::try_from(value.encoded_bytes())
                .map_err(|_| ProjectionEvidenceRefusal::DomainEncoding)?,
        )?;
        out.raw(&value.target_representation_digest())?;
        out.u64(value.spent_work())?;
        out.u64(value.admitted_work())
    }
    fn target(
        &self,
        value: &MathematicalQuantityView<'target>,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.bytes(value.encoded())
    }
    fn detail(
        &self,
        _: &(),
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.raw(&[])
    }
    fn policy(
        &self,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.bytes(&self.policy.target_unit.encode())
    }
}
