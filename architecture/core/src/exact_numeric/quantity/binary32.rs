//! Binary32 projection retains the independent physical coordinate contract.
use super::RationalQuantity;
use crate::{
    projection::*, Binary32ErrorClaim, Binary32ProjectionDomain, Binary32ProjectionPolicy,
    Binary32Refusal, Binary32Rounding, ExactNumericWork, IeeeF32, PreparedBinary32Projection,
    QuantityRole, Unit,
};
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantityBinary32ProjectionRefusal {
    Numeric(Binary32Refusal),
    PhysicalToleranceMismatch,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct QuantityBinary32 {
    coordinate: IeeeF32,
    unit: Unit,
    role: QuantityRole,
}
impl QuantityBinary32 {
    pub const fn coordinate(self) -> IeeeF32 {
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
pub struct PreparedQuantityBinary32Projection {
    source: RationalQuantity,
    numeric: PreparedBinary32Projection,
    target: QuantityBinary32,
    maximum_error: RationalQuantity,
}
impl PreparedQuantityBinary32Projection {
    /// Tolerance is a difference in the exact source Unit: Linear for linear
    /// families, Delta for affine points/deltas. Convert its Unit explicitly
    /// before preparation; an offset point is never an error distance.
    pub fn prepare(
        source: RationalQuantity,
        rounding: Binary32Rounding,
        maximum_error: RationalQuantity,
        work: &mut ExactNumericWork,
    ) -> Result<Self, QuantityBinary32ProjectionRefusal> {
        let error_role = if source.role() == QuantityRole::Point {
            QuantityRole::Delta
        } else {
            source.role()
        };
        if maximum_error.unit() != source.unit() || maximum_error.role() != error_role {
            return Err(QuantityBinary32ProjectionRefusal::PhysicalToleranceMismatch);
        }
        let numeric = PreparedBinary32Projection::prepare(
            source.coordinate(),
            rounding,
            maximum_error.coordinate(),
            work,
        )
        .map_err(QuantityBinary32ProjectionRefusal::Numeric)?;
        Ok(Self {
            source,
            numeric,
            target: QuantityBinary32 {
                coordinate: numeric.target(),
                unit: source.unit(),
                role: source.role(),
            },
            maximum_error,
        })
    }
    pub const fn numeric(&self) -> &PreparedBinary32Projection {
        &self.numeric
    }
    pub const fn source(self) -> RationalQuantity {
        self.source
    }
    pub const fn target(self) -> QuantityBinary32 {
        self.target
    }
    pub const fn error(self) -> Binary32ErrorClaim {
        self.numeric.error()
    }
    pub const fn maximum_error(self) -> RationalQuantity {
        self.maximum_error
    }
    pub fn facts(&self) -> [ProjectionFact<'_, QuantityBinary32ProjectionDomain>; 3] {
        let [value, representation] = self.numeric.projection_facts();
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
    fact: ProjectionFact<'_, Binary32ProjectionDomain>,
) -> ProjectionFact<'_, QuantityBinary32ProjectionDomain> {
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
pub struct QuantityBinary32ProjectionDomain;
impl ProjectionDomain for QuantityBinary32ProjectionDomain {
    type Source = PreparedQuantityBinary32Projection;
    type Target = QuantityBinary32;
    type Detail = Binary32ErrorClaim;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(super::RATIONAL_QUANTITY_ID)
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("numeric/quantity-binary32-projection@1")
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
        let mut numeric: [ProjectionFact<'_, Binary32ProjectionDomain>; 2] =
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
        Binary32ProjectionDomain.validate(
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
pub struct QuantityBinary32ProjectionPolicy {
    pub maximum_error: RationalQuantity,
    pub rounding: Binary32Rounding,
}
impl ProjectionPolicy<QuantityBinary32ProjectionDomain> for QuantityBinary32ProjectionPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/quantity-binary32-policy@1")
    }
    fn configuration_digest(&self) -> Option<[u8; 32]> {
        let mut bytes = [0; 33];
        bytes[..32].copy_from_slice(&self.maximum_error.semantic_digest());
        bytes[32] = match self.rounding {
            Binary32Rounding::NearestTiesEven => 0,
            Binary32Rounding::TowardZero => 1,
            Binary32Rounding::TowardPositive => 2,
            Binary32Rounding::TowardNegative => 3,
        };
        Some(crate::semantic_digest(
            "numeric/quantity-binary32-policy-config@1",
            &bytes,
        ))
    }
    fn permits(
        &self,
        source: &PreparedQuantityBinary32Projection,
        target: &QuantityBinary32,
        facts: &[ProjectionFact<'_, QuantityBinary32ProjectionDomain>],
    ) -> bool {
        self.maximum_error.unit() == source.maximum_error.unit()
            && self.maximum_error.role() == source.maximum_error.role()
            && Binary32ProjectionPolicy {
                maximum_error: self.maximum_error.coordinate(),
                rounding: self.rounding,
            }
            .permits(
                &source.numeric,
                &target.coordinate,
                &source.numeric.projection_facts(),
            )
            && QuantityBinary32ProjectionDomain.validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
