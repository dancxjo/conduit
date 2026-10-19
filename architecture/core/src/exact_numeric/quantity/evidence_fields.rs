//! Shared physical numeric fields for the general evidence writer.
use super::*;
use crate::projection::*;
use crate::{Binary32ErrorClaim, Binary32Rounding, EXACT_RATIONAL_MAX_ENCODED_BYTES};
type E = ProjectionEvidenceRefusal;
type Writer<'a> = ProjectionEvidenceWriter<'a>;
pub(super) const RATIONAL_FIELD: usize = 4 + crate::EXACT_RATIONAL_MAX_ENCODED_BYTES;
pub(super) const PHYSICAL_FIELD: usize = 4 + crate::UNIT_ENCODED_LEN + 1;
pub(super) fn rational(value: ExactRational, out: &mut Writer<'_>) -> Result<(), E> {
    let mut bytes = [0; EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let length = value.encode(&mut bytes).map_err(|_| E::DomainEncoding)?;
    out.bytes(&bytes[..length])
}
pub(super) fn physical(unit: Unit, role: QuantityRole, out: &mut Writer<'_>) -> Result<(), E> {
    out.bytes(&unit.encode())?;
    out.tag(match role {
        QuantityRole::Linear => 0,
        QuantityRole::Point => 1,
        QuantityRole::Delta => 2,
    })
}
pub(super) fn quantity(value: RationalQuantity, out: &mut Writer<'_>) -> Result<(), E> {
    rational(value.coordinate(), out)?;
    physical(value.unit(), value.role(), out)
}
pub(super) fn target(value: QuantityBinary32, out: &mut Writer<'_>) -> Result<(), E> {
    out.u32(value.coordinate().bits())?;
    physical(value.unit(), value.role(), out)
}
pub(super) fn rounding(value: Binary32Rounding, out: &mut Writer<'_>) -> Result<(), E> {
    out.tag(match value {
        Binary32Rounding::NearestTiesEven => 0,
        Binary32Rounding::TowardZero => 1,
        Binary32Rounding::TowardPositive => 2,
        Binary32Rounding::TowardNegative => 3,
    })
}
pub(super) fn error(value: Binary32ErrorClaim, out: &mut Writer<'_>) -> Result<(), E> {
    let (tag, amount) = match value {
        Binary32ErrorClaim::ExactDistance(q) => (0, q),
        Binary32ErrorClaim::CertifiedBound(q) => (1, q),
    };
    out.tag(tag)?;
    rational(amount, out)
}
