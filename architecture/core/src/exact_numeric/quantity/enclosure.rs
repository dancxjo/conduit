//! Physical coordinate enclosures retain Unit/family/role and typed tolerance.
use super::RationalQuantity;
use crate::{
    projection::*, ExactNumericWork, IntegerEnclosure, IntegerEnclosureDomain,
    IntegerEnclosureError, IntegerEnclosurePolicy, IntegerEnclosureRefusal,
    PreparedIntegerEnclosure, PrimitiveInfoKind, QuantityRole, Unit,
};
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantityIntegerEnclosureRefusal {
    Numeric(IntegerEnclosureRefusal),
    PhysicalToleranceMismatch,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct QuantityIntegerEnclosure {
    coordinate: IntegerEnclosure,
    unit: Unit,
    role: QuantityRole,
}
impl QuantityIntegerEnclosure {
    pub const fn coordinate(self) -> IntegerEnclosure {
        self.coordinate
    }
    pub const fn unit(self) -> Unit {
        self.unit
    }
    pub const fn role(self) -> QuantityRole {
        self.role
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PreparedQuantityIntegerEnclosure {
    source: RationalQuantity,
    numeric: PreparedIntegerEnclosure,
    target: QuantityIntegerEnclosure,
    maximum_outward_error: RationalQuantity,
}
impl PreparedQuantityIntegerEnclosure {
    /// Tolerance is a difference in the exact source Unit: Linear for linear
    /// families, Delta for affine points/deltas. Convert its Unit explicitly
    /// before preparation; an offset point is never an error distance.
    pub fn prepare(
        source: RationalQuantity,
        kind: PrimitiveInfoKind,
        maximum_outward_error: RationalQuantity,
        work: &mut ExactNumericWork,
    ) -> Result<Self, QuantityIntegerEnclosureRefusal> {
        let error_role = if source.role() == QuantityRole::Point {
            QuantityRole::Delta
        } else {
            source.role()
        };
        if maximum_outward_error.unit() != source.unit()
            || maximum_outward_error.role() != error_role
        {
            return Err(QuantityIntegerEnclosureRefusal::PhysicalToleranceMismatch);
        }
        let numeric = PreparedIntegerEnclosure::prepare(
            source.coordinate(),
            kind,
            maximum_outward_error.coordinate(),
            work,
        )
        .map_err(QuantityIntegerEnclosureRefusal::Numeric)?;
        Ok(Self {
            source,
            numeric,
            target: QuantityIntegerEnclosure {
                coordinate: numeric.target(),
                unit: source.unit(),
                role: source.role(),
            },
            maximum_outward_error,
        })
    }
    pub const fn source(self) -> RationalQuantity {
        self.source
    }
    pub const fn target(self) -> QuantityIntegerEnclosure {
        self.target
    }
    pub const fn error(self) -> IntegerEnclosureError {
        self.numeric.error()
    }
    pub const fn maximum_outward_error(self) -> RationalQuantity {
        self.maximum_outward_error
    }
    pub fn facts(&self) -> [ProjectionFact<'_, QuantityIntegerEnclosureDomain>; 3] {
        let [value, representation] = self.numeric.facts();
        [
            physical_fact(value),
            physical_fact(representation),
            ProjectionFact::Preserved {
                obligation: text(PHYSICAL),
            },
        ]
    }
}
fn physical_fact(
    fact: ProjectionFact<'_, IntegerEnclosureDomain>,
) -> ProjectionFact<'_, QuantityIntegerEnclosureDomain> {
    match fact {
        ProjectionFact::Preserved { obligation } => ProjectionFact::Preserved { obligation },
        ProjectionFact::Transformed { obligation, law } => {
            ProjectionFact::Transformed { obligation, law }
        }
        ProjectionFact::Lost {
            obligation,
            class,
            detail,
            native_fact,
        } => ProjectionFact::Lost {
            obligation,
            class,
            detail,
            native_fact,
        },
    }
}
const PHYSICAL: &str = "physical-coordinate-contract";
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).expect("reviewed bounded identity")
}
#[derive(Debug, Copy, Clone)]
pub struct QuantityIntegerEnclosureDomain;
impl ProjectionDomain for QuantityIntegerEnclosureDomain {
    type Source = PreparedQuantityIntegerEnclosure;
    type Target = QuantityIntegerEnclosure;
    type Detail = IntegerEnclosureError;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(super::RATIONAL_QUANTITY_ID)
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("numeric/quantity-integer-enclosure@1")
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
        if target != Some(&source.target) || facts.len() != 3 {
            return false;
        }
        let Some(physical) = facts.iter().position(|fact| matches!(fact, ProjectionFact::Preserved { obligation } if obligation.as_str() == PHYSICAL)) else { return false; };
        let mut numeric: [ProjectionFact<'_, IntegerEnclosureDomain>; 2] =
            core::array::from_fn(|_| ProjectionFact::Preserved {
                obligation: text(PHYSICAL),
            });
        let mut index = 0;
        for (position, fact) in facts.iter().enumerate() {
            if position == physical {
                continue;
            }
            numeric[index] = match fact {
                ProjectionFact::Preserved { obligation } => ProjectionFact::Preserved {
                    obligation: *obligation,
                },
                ProjectionFact::Transformed { obligation, law } => ProjectionFact::Transformed {
                    obligation: *obligation,
                    law: *law,
                },
                ProjectionFact::Lost {
                    obligation,
                    class,
                    detail,
                    native_fact,
                } => ProjectionFact::Lost {
                    obligation: *obligation,
                    class: *class,
                    detail,
                    native_fact: *native_fact,
                },
            };
            index += 1;
        }
        IntegerEnclosureDomain.validate(
            &source.numeric,
            Some(&source.target.coordinate),
            &numeric,
            native,
            scores,
            mechanism,
        )
    }
}
#[derive(Debug, Copy, Clone)]
pub struct QuantityIntegerEnclosurePolicy {
    pub maximum_outward_error: RationalQuantity,
}
impl ProjectionPolicy<QuantityIntegerEnclosureDomain> for QuantityIntegerEnclosurePolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/quantity-integer-enclosure-policy@1")
    }
    fn permits(
        &self,
        source: &PreparedQuantityIntegerEnclosure,
        target: &QuantityIntegerEnclosure,
        facts: &[ProjectionFact<'_, QuantityIntegerEnclosureDomain>],
    ) -> bool {
        self.maximum_outward_error == source.maximum_outward_error
            && IntegerEnclosurePolicy {
                maximum_outward_error: self.maximum_outward_error.coordinate(),
            }
            .permits(&source.numeric, &target.coordinate, &source.numeric.facts())
            && QuantityIntegerEnclosureDomain.validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
