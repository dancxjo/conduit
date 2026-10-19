//! Existing projection evidence extended with the retained physical frame.
use super::BorrowedMathematicalQuantityEnclosure;
use crate::{
    projection::*, BorrowedMathematicalEnclosureDomain, ExactRational,
    MathematicalEnclosureRequirements, MathematicalInterval, QuantityRole, Unit,
};
use core::marker::PhantomData;

/// Coordinate bounds in the original Unit, never an implicit Unit conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MathematicalQuantityInterval {
    pub coordinate: MathematicalInterval,
    pub unit: Unit,
    pub role: QuantityRole,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct MathematicalQuantityEnclosureDomain<'source>(PhantomData<&'source [u8]>);
impl MathematicalQuantityEnclosureDomain<'_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
const PHYSICAL: &str = "physical-coordinate-contract";
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).expect("reviewed bounded projection identity")
}
impl<'source> BorrowedMathematicalQuantityEnclosure<'source> {
    pub fn target(&self) -> MathematicalQuantityInterval {
        MathematicalQuantityInterval {
            coordinate: self.numerical().target(),
            unit: self.source().unit(),
            role: self.source().role(),
        }
    }
    pub fn facts(&self) -> [ProjectionFact<'_, MathematicalQuantityEnclosureDomain<'source>>; 3] {
        let [value, representation] = self.numerical().facts();
        [
            physical_fact(value),
            physical_fact(representation),
            ProjectionFact::Preserved {
                obligation: text(PHYSICAL),
            },
        ]
    }
}
fn physical_fact<'a, 'source>(
    fact: ProjectionFact<'a, BorrowedMathematicalEnclosureDomain<'source>>,
) -> ProjectionFact<'a, MathematicalQuantityEnclosureDomain<'source>> {
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
impl<'source> ProjectionDomain for MathematicalQuantityEnclosureDomain<'source> {
    type Source = BorrowedMathematicalQuantityEnclosure<'source>;
    type Target = MathematicalQuantityInterval;
    type Detail = ExactRational;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(crate::QUANTITY_INFO_ID)
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("numeric/quantity-coordinate-interval@1")
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
        if target != Some(&source.target()) || facts.len() != 3 {
            return false;
        }
        let Some(physical) = facts.iter().position(|fact| {
            matches!(fact,
            ProjectionFact::Preserved { obligation } if obligation.as_str() == PHYSICAL)
        }) else {
            return false;
        };
        let mut numeric: [ProjectionFact<'_, BorrowedMathematicalEnclosureDomain<'source>>; 2] =
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
        BorrowedMathematicalEnclosureDomain::new().validate(
            source.numerical(),
            target.map(|t| &t.coordinate),
            &numeric,
            native,
            scores,
            mechanism,
        )
    }
}
#[derive(Debug, Clone, Copy)]
pub struct MathematicalQuantityEnclosurePolicy {
    /// Numerical coordinate width in the retained Unit; not an affine point.
    pub requirements: MathematicalEnclosureRequirements,
}
impl<'source> ProjectionPolicy<MathematicalQuantityEnclosureDomain<'source>>
    for MathematicalQuantityEnclosurePolicy
{
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/quantity-coordinate-interval-policy@1")
    }
    fn permits(
        &self,
        source: &BorrowedMathematicalQuantityEnclosure<'source>,
        target: &MathematicalQuantityInterval,
        facts: &[ProjectionFact<'_, MathematicalQuantityEnclosureDomain<'source>>],
    ) -> bool {
        self.requirements == source.numerical().requirements()
            && MathematicalQuantityEnclosureDomain::new().validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
