//! Numerical fidelity uses the shared projection contract. Expensive certificate
//! verification happens in preparation; report admission is borrowed and pure.
use super::*;
use crate::numeric_realization::{NumericEngineRefusal, VerifiedNumericWitness};
use crate::projection::{
    ProjectionDomain, ProjectionFact, ProjectionLoss, ProjectionMechanism, ProjectionNativeFact,
    ProjectionPolicy, ProjectionScore, ProjectionText,
};

pub const NUMERIC_PROJECTION_OBLIGATIONS: [&str; 6] = [
    "source-denotation",
    "ordered-computation",
    "numerical-fidelity",
    "range-assumptions",
    "operational-exceptions",
    "measurement-uncertainty",
];
fn text(value: &'static str) -> ProjectionText<'static> {
    ProjectionText::new(value).expect("reviewed finite projection vocabulary")
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericProjectionSource {
    graph: [u8; 32],
    requirement: [u8; 32],
    semantics: NumericSemantics,
    // Custody of an observation's uncertainty is separate from arithmetic error.
    uncertainty: Option<[u8; 32]>,
}
impl NumericProjectionSource {
    pub fn new(
        graph: &crate::NumericComputation,
        requirement: &NumericRequirement,
    ) -> Result<Self, NumericEngineRefusal> {
        graph
            .validate()
            .map_err(|_| NumericEngineRefusal::InvalidComputation)?;
        let graph = graph
            .identity()
            .map_err(|_| NumericEngineRefusal::InvalidComputation)?;
        Ok(Self {
            graph,
            requirement: requirement.identity()?,
            semantics: requirement.semantics,
            uncertainty: None,
        })
    }
    /// Preserve the source observation and its interval. This does not claim the
    /// encoded reading is physically exact or consume the arithmetic allowance.
    pub fn with_measurement_uncertainty(
        mut self,
        observation: [u8; 32],
        uncertainty: &NumericMeasurementUncertainty,
    ) -> Result<Self, NumericRefusal> {
        uncertainty.validate()?;
        let lower = uncertainty.lower.canonical_bytes()?;
        let upper = uncertainty.upper.canonical_bytes()?;
        self.uncertainty = Some(crate::semantic_digest_parts(
            "numeric/measurement-uncertainty@1",
            &[&observation, &lower, &upper],
        ));
        Ok(self)
    }
    pub fn graph_identity(&self) -> &[u8; 32] {
        &self.graph
    }
    pub fn requirement_identity(&self) -> &[u8; 32] {
        &self.requirement
    }
    pub fn measurement_uncertainty_identity(&self) -> Option<&[u8; 32]> {
        self.uncertainty.as_ref()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericProjectionLoss {
    CertifiedRounding,
    CertifiedEnclosure,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericProjectionDetail {
    loss: NumericProjectionLoss,
    graph: [u8; 32],
    requirement: [u8; 32],
}
impl NumericProjectionDetail {
    pub fn loss(&self) -> NumericProjectionLoss {
        self.loss
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericProjectionTarget {
    witness: VerifiedNumericWitness,
    uncertainty: Option<[u8; 32]>,
    detail: Option<NumericProjectionDetail>,
}
impl NumericProjectionTarget {
    pub fn new(
        source: &NumericProjectionSource,
        witness: VerifiedNumericWitness,
    ) -> Result<Self, NumericEngineRefusal> {
        if source.graph_identity() != witness.graph_identity()
            || source.requirement_identity() != witness.requirement_identity()
            || source.semantics != witness.requirement().semantics
        {
            return Err(NumericEngineRefusal::ClaimMismatch);
        }
        let claim = witness.claim();
        let loss =
            if claim.error.lower() != &MathValue::ZERO || claim.error.upper() != &MathValue::ZERO {
                Some(NumericProjectionLoss::CertifiedRounding)
            } else if claim
                .result
                .interval()
                .is_some_and(|interval| interval.lower() != interval.upper())
            {
                Some(NumericProjectionLoss::CertifiedEnclosure)
            } else {
                None
            };
        let detail = loss.map(|loss| NumericProjectionDetail {
            loss,
            graph: source.graph,
            requirement: source.requirement,
        });
        Ok(Self {
            witness,
            uncertainty: source.uncertainty,
            detail,
        })
    }
    pub fn witness(&self) -> &VerifiedNumericWitness {
        &self.witness
    }
    pub fn loss_detail(&self) -> Option<&NumericProjectionDetail> {
        self.detail.as_ref()
    }
    pub fn measurement_uncertainty_identity(&self) -> Option<&[u8; 32]> {
        self.uncertainty.as_ref()
    }
    pub fn projection_facts(&self) -> [ProjectionFact<'_, NumericProjectionDomain>; 6] {
        let precision = match self.detail.as_ref() {
            None => ProjectionFact::Preserved {
                obligation: text(NUMERIC_PROJECTION_OBLIGATIONS[2]),
            },
            Some(detail) => ProjectionFact::Lost {
                obligation: text(NUMERIC_PROJECTION_OBLIGATIONS[2]),
                class: match detail.loss {
                    NumericProjectionLoss::CertifiedRounding => ProjectionLoss::Precision,
                    NumericProjectionLoss::CertifiedEnclosure => ProjectionLoss::Approximation,
                },
                detail,
                native_fact: None,
            },
        };
        [
            ProjectionFact::Preserved {
                obligation: text(NUMERIC_PROJECTION_OBLIGATIONS[0]),
            },
            ProjectionFact::Transformed {
                obligation: text(NUMERIC_PROJECTION_OBLIGATIONS[1]),
                law: text(self.witness.claim().algorithm.identity()),
            },
            precision,
            ProjectionFact::Preserved {
                obligation: text(NUMERIC_PROJECTION_OBLIGATIONS[3]),
            },
            ProjectionFact::Preserved {
                obligation: text(NUMERIC_PROJECTION_OBLIGATIONS[4]),
            },
            ProjectionFact::Preserved {
                obligation: text(NUMERIC_PROJECTION_OBLIGATIONS[5]),
            },
        ]
    }
}
#[derive(Debug, Clone, Copy)]
pub struct NumericProjectionDomain {
    semantics: NumericSemantics,
}
impl NumericProjectionDomain {
    pub fn new(semantics: NumericSemantics) -> Self {
        Self { semantics }
    }
}
fn correlated(source: &NumericProjectionSource, target: &NumericProjectionTarget) -> bool {
    source.graph_identity() == target.witness.graph_identity()
        && source.requirement_identity() == target.witness.requirement_identity()
        && source.semantics == target.witness.requirement().semantics
        && source.uncertainty == target.uncertainty
}
impl ProjectionDomain for NumericProjectionDomain {
    type Source = NumericProjectionSource;
    type Target = NumericProjectionTarget;
    type Detail = NumericProjectionDetail;
    fn source_contract(&self) -> ProjectionText<'_> {
        text(match self.semantics {
            NumericSemantics::Mathematical => "numeric/mathematical-computation@1",
            NumericSemantics::IeeeOperational => "numeric/ieee-operational-computation@1",
        })
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("numeric/verified-realization@1")
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
        let Some(target) = target else {
            return false;
        };
        if mechanism != ProjectionMechanism::Completed
            || source.semantics != self.semantics
            || !correlated(source, target)
            || facts.len() != NUMERIC_PROJECTION_OBLIGATIONS.len()
            || !native.is_empty()
            || !scores.is_empty()
        {
            return false;
        }
        // The report cannot omit rounding, substitute a request for proof, or
        // call an enclosure a point. Every obligation must occur exactly once.
        for (index, name) in NUMERIC_PROJECTION_OBLIGATIONS.iter().enumerate() {
            let mut matching = facts
                .iter()
                .filter(|fact| fact.obligation().as_str() == *name);
            let Some(fact) = matching.next() else {
                return false;
            };
            if matching.next().is_some() {
                return false;
            }
            let valid = match (index, fact) {
                (1, ProjectionFact::Transformed { law, .. }) => {
                    law.as_str() == target.witness.claim().algorithm.identity()
                }
                (2, ProjectionFact::Preserved { .. }) => target.detail.is_none(),
                (
                    2,
                    ProjectionFact::Lost {
                        class,
                        detail,
                        native_fact,
                        ..
                    },
                ) => {
                    native_fact.is_none()
                        && target.detail.as_ref() == Some(*detail)
                        && *class
                            == match detail.loss {
                                NumericProjectionLoss::CertifiedRounding => {
                                    ProjectionLoss::Precision
                                }
                                NumericProjectionLoss::CertifiedEnclosure => {
                                    ProjectionLoss::Approximation
                                }
                            }
                }
                (0 | 3 | 4 | 5, ProjectionFact::Preserved { .. }) => true,
                _ => false,
            };
            if !valid {
                return false;
            }
        }
        true
    }
}
/// The engine proved the exact requested policy during preparation. This policy
/// only consumes that private checked witness; it does no fresh heap arithmetic.
#[derive(Debug, Clone, Copy)]
pub struct NumericProjectionPolicy;
impl ProjectionPolicy<NumericProjectionDomain> for NumericProjectionPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        text("numeric/consumer-requirement-policy@1")
    }
    fn permits(
        &self,
        source: &NumericProjectionSource,
        target: &NumericProjectionTarget,
        _facts: &[ProjectionFact<'_, NumericProjectionDomain>],
    ) -> bool {
        correlated(source, target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::numeric_realization::{certify_computation, Binary64Rounding};
    use crate::projection::{
        ProjectionBasis, ProjectionDisposition, ProjectionFidelity, ProjectionInput,
        ProjectionRefusal, ProjectionReport,
    };
    use crate::{
        NumericComputation, NumericComputationNode, NumericComputationOperation, NumericOperation,
    };
    use alloc::{string::ToString, vec, vec::Vec};
    fn computation() -> NumericComputation {
        let literal = |value| {
            NumericComputationNode::literal(
                crate::MATH_VALUE_INFO_ID.to_string(),
                None,
                &MathValue::integer(value),
            )
            .unwrap()
        };
        let apply = |operation, operands| NumericComputationNode {
            semantic_kind: crate::MATH_VALUE_INFO_ID.to_string(),
            unit: None,
            reference_unit: None,
            operation: NumericComputationOperation::Apply {
                operation,
                operands,
            },
        };
        NumericComputation::new(
            vec![
                literal(10_000_000_000_000_000),
                literal(1),
                apply(NumericOperation::Add, vec![0, 1]),
                apply(NumericOperation::Subtract, vec![2, 0]),
            ],
            3,
        )
        .unwrap()
    }
    fn requirement() -> NumericRequirement {
        NumericRequirement {
            semantics: NumericSemantics::Mathematical,
            representation: NumericRepresentation::Binary64,
            rounding: Binary64Rounding::NearestEven,
            accuracy: NumericAccuracy::Absolute(MathValue::integer(1)),
            parameters: Vec::new(),
            budget: NumericExecutionBudget {
                storage_bytes: 1_048_576,
                work_steps: 100_000,
                limbs: 128,
                symbolic_nodes: 128,
            },
            reproducible: true,
            exceptions: NumericExceptionPolicy {
                inexact: true,
                ..Default::default()
            },
        }
    }
    fn report<'a>(
        source: &'a NumericProjectionSource,
        target: &'a NumericProjectionTarget,
        domain: &'a NumericProjectionDomain,
        policy: &'a NumericProjectionPolicy,
        facts: &'a [ProjectionFact<'a, NumericProjectionDomain>],
    ) -> Result<ProjectionReport<'a, NumericProjectionDomain>, ProjectionRefusal> {
        ProjectionReport::new(
            domain,
            policy,
            ProjectionInput {
                basis: ProjectionBasis {
                    report: text("report/numerical"),
                    source: text("source/computation"),
                    target: Some(text("target/binary64")),
                    projector: text("numeric/verified-engine@1"),
                    requested_route: text("numeric/consumer-requirement@1"),
                    boundary: None,
                },
                source,
                target: Some(target),
                facts,
                native: &[],
                scores: &[],
                admitted: &[],
                attempts: &[],
                selected_attempt: None,
                mechanism: ProjectionMechanism::Completed,
                diagnostics: &[],
            },
        )
    }
    #[test]
    fn shared_projection_report_retains_whole_computation_rounding_loss() {
        let graph = computation();
        let requirement = requirement();
        let source = NumericProjectionSource::new(&graph, &requirement).unwrap();
        let target = NumericProjectionTarget::new(
            &source,
            certify_computation(&graph, &requirement).unwrap(),
        )
        .unwrap();
        let domain = NumericProjectionDomain::new(NumericSemantics::Mathematical);
        let policy = NumericProjectionPolicy;
        let mut facts = target.projection_facts();
        let summary = report(&source, &target, &domain, &policy, &facts)
            .unwrap()
            .summary();
        assert_eq!(
            summary.disposition,
            ProjectionDisposition::Completed(ProjectionFidelity::PermittedLossy)
        );
        assert_eq!(summary.losses[ProjectionLoss::Precision as usize], 1);
        assert_eq!(
            report(&source, &target, &domain, &policy, &facts[..5]).err(),
            Some(ProjectionRefusal::Domain)
        );
        facts[2] = ProjectionFact::Preserved {
            obligation: text("numerical-fidelity"),
        };
        assert_eq!(
            report(&source, &target, &domain, &policy, &facts).err(),
            Some(ProjectionRefusal::Domain)
        );
    }
    #[test]
    fn observation_uncertainty_survives_without_paying_arithmetic_error() {
        let graph = computation();
        let mut requirement = requirement();
        let uncertainty = NumericMeasurementUncertainty {
            lower: MathValue::integer(-100),
            upper: MathValue::integer(100),
        };
        let source = NumericProjectionSource::new(&graph, &requirement)
            .unwrap()
            .with_measurement_uncertainty([3; 32], &uncertainty)
            .unwrap();
        let target = NumericProjectionTarget::new(
            &source,
            certify_computation(&graph, &requirement).unwrap(),
        )
        .unwrap();
        assert_eq!(
            source.measurement_uncertainty_identity(),
            target.measurement_uncertainty_identity()
        );
        requirement.accuracy = NumericAccuracy::Exact;
        assert!(certify_computation(&graph, &requirement).is_err());
        let changed = NumericProjectionSource::new(&graph, &requirement).unwrap();
        assert_eq!(
            NumericProjectionTarget::new(&changed, target.witness.clone()),
            Err(NumericEngineRefusal::ClaimMismatch)
        );
    }
}
