//! Exact cyclic range reduction before any machine representation choice.
use super::*;
impl ExactRational {
    pub(crate) fn quarter_turn_index(self, work: &mut Work) -> Result<Option<u8>, R> {
        let q = self.reduced_turn(work)?;
        // A reduced fraction is a quarter turn iff its denominator divides four.
        // Do not multiply an otherwise valid 4096-bit numerator by four merely
        // to discover that a non-cardinal fraction is not integral.
        Ok(if q.denominator == Natural::ONE {
            Some(0)
        } else if q.denominator == Natural::from_u128(2) {
            Some(2)
        } else if q.denominator == Natural::from_u128(4) {
            Some(u8::try_from(q.numerator.to_u128()?).map_err(|_| R::OutOfRange)?)
        } else {
            None
        })
    }

    pub(crate) fn reduced_turn(self, work: &mut Work) -> Result<Self, R> {
        let (_, remainder) = self.numerator.divide(self.denominator, work)?;
        let fraction = Self::normalized(false, remainder, self.denominator, work)?;
        if self.negative && fraction != Self::ZERO {
            Self::ONE.checked_subtract(fraction, work)
        } else {
            Ok(fraction)
        }
    }
}
