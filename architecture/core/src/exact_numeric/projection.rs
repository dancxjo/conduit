//! Exact fixed-width projection, using the existing generic fidelity report.
//! Approximate float eligibility requires a separately reviewed error witness.
use super::{ExactNumericRefusal, ExactRational};
use crate::{projection::*, FixedInteger, FixedIntegerRefusal, PrimitiveInfoKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactIntegerProjectionRefusal {
    Numeric(ExactNumericRefusal),
    Target(FixedIntegerRefusal),
    UnsupportedTarget,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactIntegerProjectionRequest {
    pub value: ExactRational,
    pub target_kind: PrimitiveInfoKind,
}
impl ExactIntegerProjectionRequest {
    pub fn project(self) -> Result<FixedInteger, ExactIntegerProjectionRefusal> {
        use ExactIntegerProjectionRefusal as R;
        use PrimitiveInfoKind as K;
        match self.target_kind {
            K::I8 | K::I16 | K::I32 | K::I64 | K::I128 => FixedInteger::from_signed(
                self.target_kind,
                self.value.checked_i128().map_err(R::Numeric)?,
            )
            .map_err(R::Target),
            K::U8 | K::U16 | K::U32 | K::U64 | K::U128 => FixedInteger::from_unsigned(
                self.target_kind,
                self.value.checked_u128().map_err(R::Numeric)?,
            )
            .map_err(R::Target),
            _ => Err(R::UnsupportedTarget),
        }
    }
}

/// Validates the complete exact-value and target-representation obligations.
/// It refuses fabricated results, target kinds, loss claims and undeclared laws.
/// Provenance/Plan correlation remains owned by ProjectionBasis/Realization.
#[derive(Debug, Clone, Copy)]
pub struct ExactIntegerProjectionDomain;
const VALUE: &str = "exact-denotation";
const REPRESENTATION: &str = "numeric-representation";
const LAW: &str = "exact-fixed-integer@1";
fn text(value: &str) -> ProjectionText<'_> {
    ProjectionText::new(value).expect("reviewed bounded projection name")
}
impl ProjectionDomain for ExactIntegerProjectionDomain {
    type Source = ExactIntegerProjectionRequest;
    type Target = FixedInteger;
    type Detail = ExactIntegerProjectionRefusal;
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
        if mechanism != ProjectionMechanism::Completed || !native.is_empty() || !scores.is_empty() {
            return false;
        }
        if target.is_none() || source.project().ok().as_ref() != target || facts.len() != 2 {
            return false;
        }
        let value = facts.iter().any(|fact| {
            matches!(fact,
            ProjectionFact::Preserved { obligation } if obligation.as_str() == VALUE)
        });
        let representation = facts.iter().any(|fact| {
            matches!(fact,
            ProjectionFact::Transformed { obligation, law }
                if obligation.as_str() == REPRESENTATION && law.as_str() == LAW)
        });
        value && representation
    }
}
/// Generic exact-policy adapter; no numerical-only fidelity or loss subsystem.
#[derive(Debug, Clone, Copy)]
pub struct ExactIntegerProjectionPolicy;
impl ProjectionPolicy<ExactIntegerProjectionDomain> for ExactIntegerProjectionPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/exact-integer-policy@1")
    }
    fn permits(
        &self,
        source: &ExactIntegerProjectionRequest,
        target: &FixedInteger,
        facts: &[ProjectionFact<'_, ExactIntegerProjectionDomain>],
    ) -> bool {
        ExactIntegerProjectionDomain.validate(
            source,
            Some(target),
            facts,
            &[],
            &[],
            ProjectionMechanism::Completed,
        )
    }
}
impl ExactIntegerProjectionDomain {
    pub fn facts(self) -> [ProjectionFact<'static, Self>; 2] {
        [
            ProjectionFact::Preserved {
                obligation: text(VALUE),
            },
            ProjectionFact::Transformed {
                obligation: text(REPRESENTATION),
                law: text(LAW),
            },
        ]
    }
}
