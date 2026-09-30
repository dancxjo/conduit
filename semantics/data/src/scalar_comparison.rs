use crate::ScalarComparison;

impl ScalarComparison {
    pub const fn evaluate(self, left: conduit_core::Scalar, right: conduit_core::Scalar) -> bool {
        match self {
            Self::Less => left.raw_microunits() < right.raw_microunits(),
            Self::LessOrEqual => left.raw_microunits() <= right.raw_microunits(),
            Self::Equal => left.raw_microunits() == right.raw_microunits(),
            Self::NotEqual => left.raw_microunits() != right.raw_microunits(),
            Self::GreaterOrEqual => left.raw_microunits() >= right.raw_microunits(),
            Self::Greater => left.raw_microunits() > right.raw_microunits(),
        }
    }
}
