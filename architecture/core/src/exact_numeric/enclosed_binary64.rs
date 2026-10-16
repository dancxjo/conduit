//! Certified symbolic enclosure followed by a separately inspectable machine rounding.
use super::*;
use alloc::boxed::Box;
use core::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnclosedBinary64Refusal {
    Enclosure(MathematicalEnclosureRefusal),
    Rounding(Binary64AlgorithmRefusal),
    Numeric(ExactNumericRefusal),
    NegativeTolerance,
    ToleranceExceeded,
}

/// The binary algorithm's exact source is the rational midpoint, while this
/// certificate's source is the original mathematical value. Neither substitutes
/// for the other. Preparation proves a bound from both enclosure endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedEnclosedBinary64 {
    enclosure: PreparedMathematicalEnclosure,
    rounded: PreparedBinary64Algorithm,
    maximum_error: ExactRational,
}
impl PreparedEnclosedBinary64 {
    pub fn prepare(
        source: &MathematicalValue,
        requirements: MathematicalEnclosureRequirements,
        rounding: Binary64Rounding,
        maximum_error: ExactRational,
        limits: Binary64AlgorithmLimits,
        work: &mut ExactNumericWork,
    ) -> Result<Self, EnclosedBinary64Refusal> {
        use EnclosedBinary64Refusal as R;
        if maximum_error.is_negative() {
            return Err(R::NegativeTolerance);
        }
        let enclosure = PreparedMathematicalEnclosure::prepare(source, requirements, work)
            .map_err(R::Enclosure)?;
        let interval = enclosure.target();
        let midpoint = interval
            .lower()
            .checked_add(interval.upper(), work)
            .and_then(|q| q.checked_divide(ExactRational::from_i128(2), work))
            .map_err(R::Numeric)?;
        let rounded = PreparedBinary64Algorithm::prepare(
            &[Binary64Operation::Constant(Box::new(midpoint))],
            rounding,
            maximum_error,
            limits,
            work,
        )
        .map_err(R::Rounding)?;
        let target = ExactRational::from_binary64(rounded.target(), work).map_err(R::Numeric)?;
        let lower = target
            .checked_subtract(interval.lower(), work)
            .map_err(R::Numeric)?;
        let upper = interval
            .upper()
            .checked_subtract(target, work)
            .map_err(R::Numeric)?;
        let lower = if lower.is_negative() {
            lower.negated()
        } else {
            lower
        };
        let upper = if upper.is_negative() {
            upper.negated()
        } else {
            upper
        };
        let error = if lower.checked_compare(upper, work).map_err(R::Numeric)? == Ordering::Greater
        {
            lower
        } else {
            upper
        };
        if error
            .checked_compare(maximum_error, work)
            .map_err(R::Numeric)?
            == Ordering::Greater
        {
            return Err(R::ToleranceExceeded);
        }
        Ok(Self {
            enclosure,
            rounded,
            maximum_error: error,
        })
    }
    pub fn source(&self) -> &MathematicalValue {
        self.enclosure.source()
    }
    /// The verifier first replays all spent work under its own smaller ceiling.
    /// Restore the supplier's inspected ceilings only after that replay agrees.
    pub(super) fn restore_admitted_work_delta(&mut self, delta: u64) {
        self.enclosure
            .restore_admitted_work(self.enclosure.admitted_limb_work() + delta);
        self.rounded
            .restore_admitted_work(self.rounded.admitted_limb_work() + delta);
    }
    pub fn enclosure(&self) -> &PreparedMathematicalEnclosure {
        &self.enclosure
    }
    pub fn rounded(&self) -> &PreparedBinary64Algorithm {
        &self.rounded
    }
    pub const fn maximum_error(&self) -> ExactRational {
        self.maximum_error
    }
    pub fn retained_payload_bytes(&self) -> usize {
        core::mem::size_of::<Self>() + self.enclosure.retained_payload_bytes() as usize
            - core::mem::size_of::<PreparedMathematicalEnclosure>()
            + self.rounded.retained_payload_bytes()
            - core::mem::size_of::<PreparedBinary64Algorithm>()
    }
}
