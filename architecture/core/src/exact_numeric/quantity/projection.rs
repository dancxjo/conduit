//! Numeric projection preserves the independent physical coordinate contract.
use super::RationalQuantity;
use crate::{
    projection::*, ExactIntegerProjectionDomain, ExactIntegerProjectionRefusal,
    ExactIntegerProjectionRequest, FixedInteger, PrimitiveInfoKind, QuantityRole, Unit,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RationalQuantityIntegerProjectionRequest {
    pub source: RationalQuantity,
    pub target_kind: PrimitiveInfoKind,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RationalQuantityInteger {
    coordinate: FixedInteger,
    unit: Unit,
    role: QuantityRole,
}
impl RationalQuantityInteger {
    pub const fn coordinate(self) -> FixedInteger {
        self.coordinate
    }
    pub const fn unit(self) -> Unit {
        self.unit
    }
    pub const fn role(self) -> QuantityRole {
        self.role
    }
}
impl RationalQuantityIntegerProjectionRequest {
    pub fn project(self) -> Result<RationalQuantityInteger, ExactIntegerProjectionRefusal> {
        Ok(RationalQuantityInteger {
            coordinate: ExactIntegerProjectionRequest {
                value: self.source.coordinate(),
                target_kind: self.target_kind,
            }
            .project()?,
            unit: self.source.unit(),
            role: self.source.role(),
        })
    }
}
#[derive(Debug, Copy, Clone)]
pub struct RationalQuantityIntegerProjectionDomain;
#[derive(Debug, Copy, Clone)]
pub struct RationalQuantityIntegerProjectionPolicy;

const PHYSICAL: &str = "physical-coordinate-contract";
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).expect("reviewed bounded projection name")
}
impl ProjectionDomain for RationalQuantityIntegerProjectionDomain {
    type Source = RationalQuantityIntegerProjectionRequest;
    type Target = RationalQuantityInteger;
    type Detail = ExactIntegerProjectionRefusal;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(super::RATIONAL_QUANTITY_ID)
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("numeric/rational-quantity-integer@1")
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
        if facts.len() != 3 || target.is_none() || source.project().ok().as_ref() != target {
            return false;
        }
        let Some(physical) = facts.iter().position(|fact| {
            matches!(fact, ProjectionFact::Preserved { obligation } if obligation.as_str() == PHYSICAL)
        }) else { return false; };
        // Reuse the common numeric obligations and validator, rather than
        // introducing another representation or a weaker fidelity contract.
        let mut numeric: [ProjectionFact<'_, ExactIntegerProjectionDomain>; 2] =
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
                _ => return false,
            };
            index += 1;
        }
        let numeric_source = ExactIntegerProjectionRequest {
            value: source.source.coordinate(),
            target_kind: source.target_kind,
        };
        ExactIntegerProjectionDomain.validate(
            &numeric_source,
            target.map(|value| &value.coordinate),
            &numeric,
            native,
            scores,
            mechanism,
        )
    }
}
impl RationalQuantityIntegerProjectionDomain {
    pub fn facts(self) -> [ProjectionFact<'static, Self>; 3] {
        let [value, representation] = ExactIntegerProjectionDomain.facts();
        let ProjectionFact::Preserved { obligation } = value else {
            unreachable!()
        };
        let ProjectionFact::Transformed {
            obligation: numeric,
            law,
        } = representation
        else {
            unreachable!()
        };
        [
            ProjectionFact::Preserved { obligation },
            ProjectionFact::Transformed {
                obligation: numeric,
                law,
            },
            ProjectionFact::Preserved {
                obligation: text(PHYSICAL),
            },
        ]
    }
}
impl ProjectionPolicy<RationalQuantityIntegerProjectionDomain>
    for RationalQuantityIntegerProjectionPolicy
{
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/rational-quantity-integer-policy@1")
    }
    fn permits(
        &self,
        source: &RationalQuantityIntegerProjectionRequest,
        target: &RationalQuantityInteger,
        facts: &[ProjectionFact<'_, RationalQuantityIntegerProjectionDomain>],
    ) -> bool {
        RationalQuantityIntegerProjectionDomain.validate(
            source,
            Some(target),
            facts,
            &[],
            &[],
            ProjectionMechanism::Completed,
        )
    }
}
