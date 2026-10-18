//! Tininess after precision rounding uses an unbounded exponent range.
//! The final rounded encoding alone is insufficient at the normal boundary.
use super::{Binary64Rounding, ExactNumericRefusal, ExactNumericWork, ExactRational, IeeeF64};
use core::cmp::Ordering;

pub(super) fn after_rounding(
    source: ExactRational,
    target: IeeeF64,
    rounding: Binary64Rounding,
    work: &mut ExactNumericWork,
) -> Result<bool, ExactNumericRefusal> {
    const MIN_NORMAL: u64 = 1 << 52;
    match target.bits() & !(1 << 63) {
        bits if bits < MIN_NORMAL => return Ok(true),
        MIN_NORMAL => (),
        _ => return Ok(false),
    }
    let negative = source.is_negative();
    let magnitude = if negative { source.negated() } else { source };
    let normal = ExactRational::from_binary64(IeeeF64::from_bits(MIN_NORMAL), work)?;
    let (distance_power, inclusive) = match rounding {
        Binary64Rounding::NearestTiesEven => (-1076, false),
        Binary64Rounding::TowardPositive if !negative => (-1075, true),
        Binary64Rounding::TowardNegative if negative => (-1075, true),
        _ => return Ok(magnitude.checked_compare(normal, work)? == Ordering::Less),
    };
    // Below min-normal, unbounded binary64 precision has quantum 2^-1075.
    // Nearest-even reaches min-normal at its midpoint (the even endpoint).
    // Directed rounding away from zero reaches it strictly above the quantum.
    let distance = ExactRational::from_i128(2).checked_power(distance_power, work)?;
    let threshold = normal.checked_subtract(distance, work)?;
    let comparison = magnitude.checked_compare(threshold, work)?;
    Ok(comparison == Ordering::Less || (inclusive && comparison == Ordering::Equal))
}
