//! Preserve numerical exhaustion, physical incompatibility and encoding failure.
use conduit_core::*;
fn numeric(why: ExactNumericRefusal) -> u16 {
    match why {
        ExactNumericRefusal::WorkLimit => 3,
        ExactNumericRefusal::ExtentExceeded | ExactNumericRefusal::OutOfRange => 4,
        _ => 1,
    }
}
fn rational(why: RationalQuantityRefusal) -> u16 {
    match why {
        RationalQuantityRefusal::Numeric(n) => numeric(n),
        _ => 2,
    }
}
fn mathematical(why: MathematicalValueRefusal) -> u16 {
    match why {
        MathematicalValueRefusal::Numeric(n) => numeric(n),
        MathematicalValueRefusal::NodeBound | MathematicalValueRefusal::DepthBound => 4,
        _ => 1,
    }
}
fn quantity(why: MathematicalQuantityRefusal) -> u16 {
    match why {
        MathematicalQuantityRefusal::Numeric(n) => numeric(n),
        MathematicalQuantityRefusal::Coordinate(n) => mathematical(n),
        MathematicalQuantityRefusal::Rational(n) => rational(n),
        _ => 1,
    }
}
fn encoding(why: QuantityValueRefusal) -> u16 {
    match why {
        QuantityValueRefusal::Rational(n) => rational(n),
        QuantityValueRefusal::Mathematical(n) => quantity(n),
        _ => 1,
    }
}
pub(super) fn conversion(why: QuantityCoordinateConversionRefusal) -> u16 {
    use QuantityCoordinateConversionRefusal as R;
    match why {
        R::Numeric(n) => numeric(n),
        R::Physical(n) => rational(n),
        R::Encoding(n) => encoding(n),
        R::Input(QuantityValueContractRefusal::Encoding(n)) => encoding(n),
        R::Input(_) => 1,
        R::RetainedStorageLimit | R::OutputTooSmall => 5,
        R::Mathematical(n) => match n {
            MathematicalQuantityConversionRefusal::Numeric(n) => numeric(n),
            MathematicalQuantityConversionRefusal::Quantity(n) => quantity(n),
            MathematicalQuantityConversionRefusal::Physical(n) => rational(n),
            MathematicalQuantityConversionRefusal::Mathematical(n) => mathematical(n),
            MathematicalQuantityConversionRefusal::RetainedStorageLimit => 5,
        },
    }
}
