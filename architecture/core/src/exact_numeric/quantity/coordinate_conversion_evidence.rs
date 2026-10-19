//! Full original Quantity wire and exact converted wire in the shared CPE.
use super::evidence_fields::rational;
use super::*;
use crate::projection::*;
pub const QUANTITY_COORDINATE_CONVERSION_EVIDENCE_LIMITS: ProjectionEvidenceLimits =
    ProjectionEvidenceLimits {
        source_bytes: MATHEMATICAL_QUANTITY_CONVERSION_EVIDENCE_SOURCE_MAX_BYTES + 1,
        target_bytes: 4 + MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES,
        detail_bytes: 0,
        policy_bytes: 4 + crate::UNIT_ENCODED_LEN,
        total_bytes: 131072,
    };
#[derive(Debug, Clone, Copy)]
pub struct QuantityCoordinateConversionEvidenceCodec {
    policy: QuantityCoordinateConversionPolicy,
    digest: [u8; 32],
}
impl QuantityCoordinateConversionEvidenceCodec {
    pub fn new(policy: QuantityCoordinateConversionPolicy) -> Self {
        let digest = <QuantityCoordinateConversionPolicy as ProjectionPolicy<
            QuantityCoordinateConversionDomain<'_, '_>,
        >>::configuration_digest(&policy)
        .unwrap();
        Self { policy, digest }
    }
}
impl<'source, 'target> ProjectionEvidenceCodec<QuantityCoordinateConversionDomain<'source, 'target>>
    for QuantityCoordinateConversionEvidenceCodec
{
    fn identity(&self) -> ProjectionText<'_> {
        ProjectionText::new("physical/quantity-coordinate-evidence@1").unwrap()
    }
    fn configuration_digest(&self) -> [u8; 32] {
        self.digest
    }
    fn source(
        &self,
        value: &QuantityCoordinateConversion<'source>,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.bytes(value.source().encoded())?;
        out.bytes(&value.target_unit().encode())?;
        out.tag(match value.target_representation() {
            QuantityCoordinateRepresentation::Decimal => 1,
            QuantityCoordinateRepresentation::Rational => 2,
            QuantityCoordinateRepresentation::Mathematical => 3,
        })?;
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
        value: &QuantityValueView<'target>,
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
