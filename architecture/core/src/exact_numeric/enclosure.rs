//! Certified outward integer bounds prepared under caller-admitted arithmetic.
//! Bounds preserve containment, not exact point denotation for a fraction.
use super::{ExactNumericRefusal, ExactNumericWork, ExactRational};
use crate::{projection::*, FixedInteger, FixedIntegerRefusal, PrimitiveInfoKind};
use core::cmp::Ordering;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum IntegerEnclosureRefusal {
    Numeric(ExactNumericRefusal),
    Target(FixedIntegerRefusal),
    UnsupportedTarget,
    NegativeTolerance,
    ToleranceExceeded,
}
impl From<ExactNumericRefusal> for IntegerEnclosureRefusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Numeric(value)
    }
}
impl From<FixedIntegerRefusal> for IntegerEnclosureRefusal {
    fn from(value: FixedIntegerRefusal) -> Self {
        Self::Target(value)
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct IntegerEnclosure {
    lower: FixedInteger,
    upper: FixedInteger,
}
impl IntegerEnclosure {
    pub const fn lower(self) -> FixedInteger {
        self.lower
    }
    pub const fn upper(self) -> FixedInteger {
        self.upper
    }
}
/// Arithmetic outward distances, independent of statistical/model uncertainty.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct IntegerEnclosureError {
    lower_distance: ExactRational,
    upper_distance: ExactRational,
}
impl IntegerEnclosureError {
    pub const fn lower_distance(self) -> ExactRational {
        self.lower_distance
    }
    pub const fn upper_distance(self) -> ExactRational {
        self.upper_distance
    }
}
/// Sealed prepared projection: only preparation can certify source/target/error
/// correlation. Inspection validates custody without hidden arithmetic work.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PreparedIntegerEnclosure {
    source: ExactRational,
    target: IntegerEnclosure,
    error: IntegerEnclosureError,
    maximum_outward_error: ExactRational,
}
impl PreparedIntegerEnclosure {
    pub fn prepare(
        source: ExactRational,
        kind: PrimitiveInfoKind,
        maximum_outward_error: ExactRational,
        work: &mut ExactNumericWork,
    ) -> Result<Self, IntegerEnclosureRefusal> {
        use PrimitiveInfoKind as K;
        if !matches!(
            kind,
            K::I8 | K::I16 | K::I32 | K::I64 | K::I128 | K::U8 | K::U16 | K::U32 | K::U64 | K::U128
        ) {
            return Err(IntegerEnclosureRefusal::UnsupportedTarget);
        }
        if maximum_outward_error.is_negative() {
            return Err(IntegerEnclosureRefusal::NegativeTolerance);
        }
        let (magnitude, fractional) = source.integer_magnitude(work)?;
        let outward = if fractional {
            magnitude
                .checked_add(1)
                .ok_or(ExactNumericRefusal::OutOfRange)?
        } else {
            magnitude
        };
        let (lower_magnitude, upper_magnitude) = if source.is_negative() {
            (outward, magnitude)
        } else {
            (magnitude, outward)
        };
        let (lower, lower_value) = endpoint(kind, source.is_negative(), lower_magnitude)?;
        let (upper, upper_value) = endpoint(kind, source.is_negative(), upper_magnitude)?;
        let error = IntegerEnclosureError {
            lower_distance: source.checked_subtract(lower_value, work)?,
            upper_distance: upper_value.checked_subtract(source, work)?,
        };
        for distance in [error.lower_distance, error.upper_distance] {
            if distance.checked_compare(maximum_outward_error, work)? == Ordering::Greater {
                return Err(IntegerEnclosureRefusal::ToleranceExceeded);
            }
        }
        Ok(Self {
            source,
            target: IntegerEnclosure { lower, upper },
            error,
            maximum_outward_error,
        })
    }
    pub const fn source(self) -> ExactRational {
        self.source
    }
    pub const fn target(self) -> IntegerEnclosure {
        self.target
    }
    pub const fn error(self) -> IntegerEnclosureError {
        self.error
    }
    pub const fn maximum_outward_error(self) -> ExactRational {
        self.maximum_outward_error
    }
    pub fn facts(&self) -> [ProjectionFact<'_, IntegerEnclosureDomain>; 2] {
        [
            if self.target.lower == self.target.upper {
                ProjectionFact::Preserved {
                    obligation: text(VALUE),
                }
            } else {
                ProjectionFact::Lost {
                    obligation: text(VALUE),
                    class: ProjectionLoss::Precision,
                    detail: &self.error,
                    native_fact: None,
                }
            },
            ProjectionFact::Transformed {
                obligation: text(REPRESENTATION),
                law: text(LAW),
            },
        ]
    }
}
fn endpoint(
    kind: PrimitiveInfoKind,
    negative: bool,
    magnitude: u128,
) -> Result<(FixedInteger, ExactRational), IntegerEnclosureRefusal> {
    use PrimitiveInfoKind as K;
    let rational = ExactRational::from_u128(magnitude);
    let value = if negative {
        rational.negated()
    } else {
        rational
    };
    let integer = if matches!(kind, K::I8 | K::I16 | K::I32 | K::I64 | K::I128) {
        FixedInteger::from_signed(kind, value.checked_i128()?)?
    } else {
        FixedInteger::from_unsigned(kind, value.checked_u128()?)?
    };
    Ok((integer, value))
}
const VALUE: &str = "exact-denotation";
const REPRESENTATION: &str = "numeric-representation";
const LAW: &str = "numeric/outward-integer-enclosure@1";
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).expect("reviewed bounded identity")
}
#[derive(Debug, Copy, Clone)]
pub struct IntegerEnclosureDomain;
impl ProjectionDomain for IntegerEnclosureDomain {
    type Source = PreparedIntegerEnclosure;
    type Target = IntegerEnclosure;
    type Detail = IntegerEnclosureError;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(super::EXACT_RATIONAL_ID)
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text(LAW)
    }
    fn validate(
        &self,
        source: &Self::Source,
        target: Option<&Self::Target>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool {
        if mechanism != ProjectionMechanism::Completed
            || !native.is_empty()
            || !scores.is_empty()
            || target != Some(&source.target)
            || facts.len() != 2
        {
            return false;
        }
        let value = facts.iter().any(|fact| match fact {
            ProjectionFact::Preserved { obligation } => {
                obligation.as_str() == VALUE && source.target.lower == source.target.upper
            }
            ProjectionFact::Lost {
                obligation,
                class,
                detail,
                native_fact,
            } => {
                obligation.as_str() == VALUE
                    && *class == ProjectionLoss::Precision
                    && **detail == source.error
                    && native_fact.is_none()
                    && source.target.lower != source.target.upper
            }
            _ => false,
        });
        value && facts.iter().any(|fact| matches!(fact, ProjectionFact::Transformed { obligation, law } if obligation.as_str() == REPRESENTATION && law.as_str() == LAW))
    }
}
/// The admitted coordinate-error requirement is retained in the prepared source
/// and must match the consumer policy; there is no universal lossy fallback.
#[derive(Debug, Copy, Clone)]
pub struct IntegerEnclosurePolicy {
    pub maximum_outward_error: ExactRational,
}
impl ProjectionPolicy<IntegerEnclosureDomain> for IntegerEnclosurePolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/integer-enclosure-policy@1")
    }
    fn permits(
        &self,
        source: &PreparedIntegerEnclosure,
        target: &IntegerEnclosure,
        facts: &[ProjectionFact<'_, IntegerEnclosureDomain>],
    ) -> bool {
        self.maximum_outward_error == source.maximum_outward_error
            && IntegerEnclosureDomain.validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
