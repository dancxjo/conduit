//! Exact finite-decimal projection without narrowing a scientific coordinate first.
use super::*;

impl ExactRational {
    /// Return normalized decimal parts when the reduced denominator contains only
    /// factors of two and five and the final coefficient fits i128. Intermediate
    /// limbs retain the admitted extent; every division spends the supplied work.
    pub fn checked_decimal_parts(self, work: &mut Work) -> Result<(i128, i16), R> {
        if self.numerator == Natural::ZERO {
            work.charge(1)?;
            return Ok((0, 0));
        }
        let mut coefficient = self.numerator;
        let mut denominator = self.denominator;
        let mut twos = 0i16;
        let mut fives = 0i16;
        for (factor, count) in [(2, &mut twos), (5, &mut fives)] {
            loop {
                let (quotient, remainder) = denominator.divide(Natural::from_u128(factor), work)?;
                if remainder != Natural::ZERO {
                    break;
                }
                denominator = quotient;
                *count += 1;
            }
        }
        if denominator != Natural::ONE {
            return Err(R::InexactProjection);
        }
        let places = twos.max(fives);
        for (factor, count) in [(2, places - twos), (5, places - fives)] {
            for _ in 0..count {
                coefficient = coefficient.small(factor, 0, work)?;
            }
        }
        let mut exponent = -places;
        loop {
            let (quotient, remainder) = coefficient.divide(Natural::from_u128(10), work)?;
            if remainder != Natural::ZERO {
                break;
            }
            coefficient = quotient;
            exponent += 1;
        }
        let magnitude = coefficient.to_u128()?;
        let coefficient = if self.negative {
            if magnitude == 1u128 << 127 {
                i128::MIN
            } else {
                -i128::try_from(magnitude).map_err(|_| R::OutOfRange)?
            }
        } else {
            i128::try_from(magnitude).map_err(|_| R::OutOfRange)?
        };
        Ok((coefficient, exponent))
    }
}
