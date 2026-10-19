//! A bounded symbolic enclosure plus midpoint rounding, retaining physical data.
//! This certifies absolute error, not correct rounding of the exact symbolic value.
use crate::*;
use core::cmp::Ordering;
mod projection;
pub use projection::*;

pub const MATHEMATICAL_QUANTITY_BINARY32_LAW: &str = "numeric/enclosed-midpoint-binary32@1";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathematicalQuantityBinary32Refusal {
    Enclosure(MathematicalQuantityEnclosureRefusal),
    Rounding(QuantityBinary32ProjectionRefusal),
    Numeric(ExactNumericRefusal),
    PhysicalToleranceMismatch,
    NegativeTolerance,
    ToleranceExceeded,
    RetainedStorageLimit,
}
use MathematicalQuantityBinary32Refusal as R;
pub struct PreparedMathematicalQuantityBinary32 {
    enclosure: PreparedMathematicalQuantityEnclosure,
    rounding: Binary32Rounding,
    maximum_error: RationalQuantity,
    maximum_work: u64,
    retained_payload_bytes: usize,
}
pub struct BorrowedMathematicalQuantityBinary32<'a> {
    enclosure: BorrowedMathematicalQuantityEnclosure<'a>,
    midpoint: PreparedQuantityBinary32Projection,
    error: Binary32ErrorClaim,
    maximum_error: RationalQuantity,
}
impl PreparedMathematicalQuantityBinary32 {
    pub fn new(
        requirements: MathematicalEnclosureRequirements,
        rounding: Binary32Rounding,
        maximum_error: RationalQuantity,
        maximum_work: u64,
    ) -> Result<Self, R> {
        if maximum_error.coordinate().is_negative() {
            return Err(R::NegativeTolerance);
        }
        if maximum_error.role() == QuantityRole::Point {
            return Err(R::PhysicalToleranceMismatch);
        }
        let enclosure = PreparedMathematicalQuantityEnclosure::new(requirements, maximum_work)
            .map_err(R::Enclosure)?;
        let retained_payload_bytes = core::mem::size_of::<Self>()
            .checked_add(enclosure.retained_payload_bytes())
            .and_then(|n| {
                n.checked_sub(core::mem::size_of::<PreparedMathematicalQuantityEnclosure>())
            })
            .ok_or(R::RetainedStorageLimit)?;
        if retained_payload_bytes > requirements.maximum_retained_payload_bytes as usize {
            return Err(R::RetainedStorageLimit);
        }
        Ok(Self {
            enclosure,
            rounding,
            maximum_error,
            maximum_work,
            retained_payload_bytes,
        })
    }
    /// Container and prepared enclosure payload; excludes stack, input and receipt.
    pub const fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
    pub fn execute<'a>(
        &mut self,
        encoded: &'a [u8],
        work: &mut ExactNumericWork,
    ) -> Result<BorrowedMathematicalQuantityBinary32<'a>, R> {
        let mut local = ExactNumericWork::new(work.remaining().min(self.maximum_work));
        let result = self.project(encoded, &mut local);
        work.charge(local.spent()).map_err(R::Numeric)?;
        result
    }
    fn project<'a>(
        &mut self,
        encoded: &'a [u8],
        work: &mut ExactNumericWork,
    ) -> Result<BorrowedMathematicalQuantityBinary32<'a>, R> {
        let enclosure = self
            .enclosure
            .execute(encoded, work)
            .map_err(R::Enclosure)?;
        let source = enclosure.source();
        let error_role = if source.role() == QuantityRole::Point {
            QuantityRole::Delta
        } else {
            source.role()
        };
        if self.maximum_error.unit() != source.unit() || self.maximum_error.role() != error_role {
            return Err(R::PhysicalToleranceMismatch);
        }
        let interval = enclosure.numerical().target();
        let coordinate = interval
            .lower()
            .checked_add(interval.upper(), work)
            .and_then(|v| v.checked_divide(ExactRational::from_i128(2), work))
            .map_err(R::Numeric)?;
        let midpoint = PreparedQuantityBinary32Projection::prepare(
            RationalQuantity::new(coordinate, source.unit(), source.role())
                .expect("checked physical role"),
            self.rounding,
            self.maximum_error,
            work,
        )
        .map_err(R::Rounding)?;
        let represented = midpoint.numeric().represented();
        let distance = |endpoint: ExactRational, work: &mut ExactNumericWork| {
            let v = represented.checked_subtract(endpoint, work)?;
            Ok::<_, ExactNumericRefusal>(if v.is_negative() { v.negated() } else { v })
        };
        let lower = distance(interval.lower(), work).map_err(R::Numeric)?;
        let upper = distance(interval.upper(), work).map_err(R::Numeric)?;
        let bound = if lower.checked_compare(upper, work).map_err(R::Numeric)? == Ordering::Greater
        {
            lower
        } else {
            upper
        };
        if bound
            .checked_compare(self.maximum_error.coordinate(), work)
            .map_err(R::Numeric)?
            == Ordering::Greater
        {
            return Err(R::ToleranceExceeded);
        }
        let error = if interval.lower() == interval.upper() {
            Binary32ErrorClaim::ExactDistance(bound)
        } else {
            Binary32ErrorClaim::CertifiedBound(bound)
        };
        Ok(BorrowedMathematicalQuantityBinary32 {
            enclosure,
            midpoint,
            error,
            maximum_error: self.maximum_error,
        })
    }
}
impl<'a> BorrowedMathematicalQuantityBinary32<'a> {
    pub const fn source(&self) -> &MathematicalQuantityView<'a> {
        self.enclosure.source()
    }
    pub const fn enclosure(&self) -> &BorrowedMathematicalQuantityEnclosure<'a> {
        &self.enclosure
    }
    /// The midpoint's single-round receipt does not replace the symbolic source.
    pub const fn midpoint(&self) -> &PreparedQuantityBinary32Projection {
        &self.midpoint
    }
    pub const fn target(&self) -> QuantityBinary32 {
        self.midpoint.target()
    }
    pub const fn error(&self) -> Binary32ErrorClaim {
        self.error
    }
    pub const fn maximum_error(&self) -> RationalQuantity {
        self.maximum_error
    }
}
