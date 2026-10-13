//! Machin's identity π = 16 atan(1/5) - 4 atan(1/239), using
//! exact alternating-series bounds. No stored decimal expansion is evidence.
use super::{
    ExactNumericWork as Work, ExactRational as Q, MathematicalEnclosureRefusal as R,
    MathematicalInterval as Interval,
};
use core::cmp::Ordering;
struct Arctangent {
    sum: Q,
    power: Q,
    square: Q,
    terms: u16,
}
impl Arctangent {
    fn new(denominator: i128, work: &mut Work) -> Result<Self, R> {
        let power = Q::ratio(1, denominator, work)?;
        Ok(Self {
            sum: Q::ZERO,
            power,
            square: power.checked_multiply(power, work)?,
            terms: 0,
        })
    }
    fn next_term(&self, work: &mut Work) -> Result<Q, R> {
        let magnitude = self
            .power
            .checked_divide(Q::from_i128(2 * i128::from(self.terms) + 1), work)?;
        Ok(if self.terms.is_multiple_of(2) {
            magnitude
        } else {
            magnitude.negated()
        })
    }
    fn bounds(&self, work: &mut Work) -> Result<Interval, R> {
        // For 0 < x < 1 the alternating terms decrease to zero. The remainder
        // lies between zero and the first omitted term, including its sign.
        let next = self.sum.checked_add(self.next_term(work)?, work)?;
        Ok(if self.terms.is_multiple_of(2) {
            Interval::bounds(self.sum, next)
        } else {
            Interval::bounds(next, self.sum)
        })
    }
    fn advance(&mut self, work: &mut Work) -> Result<(), R> {
        self.sum = self.sum.checked_add(self.next_term(work)?, work)?;
        self.power = self.power.checked_multiply(self.square, work)?;
        self.terms += 1;
        Ok(())
    }
}
pub(super) fn enclose(
    maximum_width: Q,
    maximum_terms: u16,
    work: &mut Work,
) -> Result<(Interval, u16), R> {
    // π is not a rational point. This particular realization cannot meet a
    // zero-width request; preserve the symbolic source for another strategy.
    if maximum_width == Q::ZERO {
        return Err(R::ExactConstantEnclosureUnsupported);
    }
    let mut fifth = Arctangent::new(5, work)?;
    let mut other = Arctangent::new(239, work)?;
    loop {
        let bounds = fifth
            .bounds(work)?
            .scale(Q::from_i128(16), work)?
            .subtract(other.bounds(work)?.scale(Q::from_i128(4), work)?, work)?;
        if bounds.width(work)?.checked_compare(maximum_width, work)? != Ordering::Greater {
            return Ok((bounds, fifth.terms));
        }
        if fifth.terms == maximum_terms {
            return Err(R::ConstantTermLimit);
        }
        fifth.advance(work)?;
        other.advance(work)?;
    }
}
