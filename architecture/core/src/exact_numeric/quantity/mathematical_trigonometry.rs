//! Exact typed trig for the supported rational-turn / rational-Pi Angle forms.
use crate::{
    AngleTrigonometricFunction, AngleTrigonometryRefusal as R, ExactNumericWork, ExactRational,
    MathematicalQuantityView, MathematicalValueRefusal, QuantityRole, RationalQuantity, Unit,
};
pub fn encode_mathematical_angle_trigonometric_denotation(
    function: AngleTrigonometricFunction,
    source: &MathematicalQuantityView<'_>,
    output: &mut [u8],
    work: &mut ExactNumericWork,
) -> Result<usize, R> {
    if source.family() != Unit::Turn.family() {
        return Err(R::NotAngle);
    }
    if source.role() != QuantityRole::Linear {
        return Err(R::UnsupportedRole);
    }
    let rational = source
        .coordinate()
        .as_rational(work)
        .map_err(R::Denotation)?;
    let turns = if rational == Some(ExactRational::ZERO) {
        ExactRational::ZERO
    } else if let Some(q) = rational {
        RationalQuantity::new(q, source.unit(), source.role())
            .and_then(|q| q.convert_to_unit(Unit::Turn, work))
            .map_err(R::Coordinate)?
            .coordinate()
    } else {
        let coefficient = source
            .coordinate()
            .rational_pi_coefficient(work)
            .map_err(R::Denotation)?
            .ok_or(R::Denotation(
                MathematicalValueRefusal::UnsupportedDomainProof,
            ))?;
        // Unit scale is separate from the exact turn/radian Pi origin law.
        // This also refuses a symbolic coordinate in a non-radian origin.
        let radian_scale =
            RationalQuantity::new(ExactRational::ONE, Unit::Radian, QuantityRole::Linear)
                .and_then(|q| q.convert_to_unit(source.unit(), work))
                .map_err(R::Coordinate)?
                .coordinate();
        coefficient
            .checked_divide(radian_scale, work)
            .and_then(|q| q.checked_divide(ExactRational::from_i128(2), work))
            .map_err(|e| R::Denotation(e.into()))?
    };
    super::super::symbolic::encode_rational_turns(function, turns, output, work)
        .map_err(R::Denotation)
}

/// Explicit optional-profile entrance for legacy rational coordinate forms.
pub fn encode_rational_angle_trigonometric_denotation(
    function: AngleTrigonometricFunction,
    source: RationalQuantity,
    output: &mut [u8],
    work: &mut ExactNumericWork,
) -> Result<usize, R> {
    if source.family() != Unit::Turn.family() {
        return Err(R::NotAngle);
    }
    if source.role() != QuantityRole::Linear {
        return Err(R::UnsupportedRole);
    }
    let turns = if source.coordinate() == ExactRational::ZERO {
        ExactRational::ZERO
    } else {
        source
            .convert_to_unit(Unit::Turn, work)
            .map_err(R::Coordinate)?
            .coordinate()
    };
    super::super::symbolic::encode_rational_turns(function, turns, output, work)
        .map_err(R::Denotation)
}
