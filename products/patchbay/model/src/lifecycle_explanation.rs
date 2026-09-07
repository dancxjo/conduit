//! Read-only projection of generic lifecycle evidence.

use conduit_body::{BodyAdministrativeResult, WorkloadTransitionState};
use conduit_core::ResourceAcquisitionState;
use conduit_kernel::{
    causal_evidence::{
        CausalEvidence, CausalEvidenceRefusal, CausalRelationship, EvidenceIdentity,
    },
    fault_disposition::FaultDisposition,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleExplanation {
    pub summary: String,
}

pub fn explain_workload(state: &WorkloadTransitionState) -> LifecycleExplanation {
    LifecycleExplanation {
        summary: format!("workload transition: {state:?}"),
    }
}
pub fn explain_acquisition(state: &ResourceAcquisitionState) -> LifecycleExplanation {
    LifecycleExplanation {
        summary: format!("resource acquisition: {state:?}"),
    }
}
pub fn explain_administration(result: &BodyAdministrativeResult) -> LifecycleExplanation {
    LifecycleExplanation {
        summary: format!(
            "administrative result {} -> revision {} ({})",
            result.request_id, result.resulting_revision, result.evidence_id
        ),
    }
}
pub fn explain_fault(disposition: FaultDisposition) -> LifecycleExplanation {
    LifecycleExplanation {
        summary: format!(
            "failure {:?} -> {:?}",
            disposition.scope, disposition.policy
        ),
    }
}
pub fn explain_cause<const N: usize>(
    evidence: &CausalEvidence<N>,
    effect: EvidenceIdentity,
    relationship: CausalRelationship,
) -> Result<LifecycleExplanation, CausalEvidenceRefusal> {
    let cause = evidence.cause_of(effect, relationship)?;
    Ok(LifecycleExplanation {
        summary: format!(
            "evidence {} {relationship:?} evidence {}",
            effect.sign, cause.sign
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::causal_evidence::{CausalEdge, CausalRelationship};

    #[test]
    fn patchbay_explains_generic_evidence_and_leaves_missing_truth_unknown() {
        let cause = EvidenceIdentity {
            sign: 1,
            execution: 1,
            host_session: 1,
        };
        let effect = EvidenceIdentity {
            sign: 2,
            execution: 1,
            host_session: 1,
        };
        let mut evidence = CausalEvidence::<1>::default();
        evidence
            .record(CausalEdge {
                cause,
                effect,
                relationship: CausalRelationship::CausedBy,
            })
            .unwrap();
        assert!(
            explain_cause(&evidence, effect, CausalRelationship::CausedBy)
                .unwrap()
                .summary
                .contains("evidence 1")
        );
        assert_eq!(
            explain_cause(&evidence, effect, CausalRelationship::Corrects),
            Err(CausalEvidenceRefusal::Unknown)
        );
    }
}
