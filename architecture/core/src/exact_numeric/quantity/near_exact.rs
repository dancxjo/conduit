//! Exact qualified coordinates use the shared report and a concrete physical allowance.
use super::*;
use crate::projection::*;
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).expect("fixed exact Near vocabulary")
}
#[derive(Debug, Clone, Copy, Default)]
pub struct ExactNearProjectionDomain;
impl ExactNearProjectionDomain {
    pub fn facts(self) -> [ProjectionFact<'static, Self>; 3] {
        [
            ProjectionFact::Preserved {
                obligation: text("mathematical-denotation"),
            },
            ProjectionFact::Transformed {
                obligation: text("numeric-representation"),
                law: text("exact-rational-qualified-coordinate@1"),
            },
            ProjectionFact::Preserved {
                obligation: text("physical-coordinate-contract"),
            },
        ]
    }
}
impl ProjectionDomain for ExactNearProjectionDomain {
    type Source = RationalQuantity;
    type Target = RationalQuantity;
    type Detail = ();
    fn source_contract(&self) -> ProjectionText<'_> {
        text(RATIONAL_QUANTITY_ID)
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("numeric/near-exact-rational@1")
    }
    fn validate(
        &self,
        source: &RationalQuantity,
        target: Option<&RationalQuantity>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool {
        target==Some(source)&&facts.len()==3&&native.is_empty()&&scores.is_empty()&&mechanism==ProjectionMechanism::Completed
            &&facts.iter().any(|f|matches!(f,ProjectionFact::Preserved{obligation} if obligation.as_str()=="mathematical-denotation"))
            &&facts.iter().any(|f|matches!(f,ProjectionFact::Preserved{obligation} if obligation.as_str()=="physical-coordinate-contract"))
            &&facts.iter().any(|f|matches!(f,ProjectionFact::Transformed{obligation,law} if obligation.as_str()=="numeric-representation"&&law.as_str()=="exact-rational-qualified-coordinate@1"))
    }
}
#[derive(Debug, Clone, Copy)]
pub struct ExactNearProjectionPolicy {
    pub maximum_error: RationalQuantity,
}
impl ProjectionPolicy<ExactNearProjectionDomain> for ExactNearProjectionPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/near-exact-rational-policy@1")
    }
    fn configuration_digest(&self) -> Option<[u8; 32]> {
        Some(crate::semantic_digest(
            "numeric/near-exact-rational-policy-config@1",
            &self.maximum_error.semantic_digest(),
        ))
    }
    fn permits(
        &self,
        source: &RationalQuantity,
        target: &RationalQuantity,
        facts: &[ProjectionFact<'_, ExactNearProjectionDomain>],
    ) -> bool {
        let role = if source.role() == QuantityRole::Point {
            QuantityRole::Delta
        } else {
            source.role()
        };
        self.maximum_error.unit() == source.unit()
            && self.maximum_error.role() == role
            && !self.maximum_error.coordinate().is_negative()
            && ExactNearProjectionDomain.validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
