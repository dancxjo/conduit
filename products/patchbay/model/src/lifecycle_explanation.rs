//! Patchbay-specific lifecycle summaries and compatibility exports for the
//! Observatory-owned causal projection.

use conduit_body::{BodyAdministrativeResult, WorkloadTransitionState};
use conduit_core::ResourceAcquisitionState;
use conduit_kernel::causal_evidence::{
    CausalEvidence, CausalEvidenceRefusal, CausalRelationship, EvidenceIdentity,
};
use conduit_kernel::fault_disposition::FaultDisposition;

pub use conduit_observatory::{
    explain_terminal, explain_trace, explain_trace_with_metadata, CausalExplanationEdge,
    CausalExplanationMetadata, CausalExplanationMetadataFact, CausalExplanationNode,
    CausalExplanationRefusal, CausalExplanationVisibility, CausalTraceExplanation,
    MAXIMUM_CAUSAL_EXPLANATION_EDGES, MAXIMUM_CAUSAL_EXPLANATION_NODES,
    MAXIMUM_CAUSAL_METADATA_FACTS_PER_NODE,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleExplanation {
    pub summary: String,
}

pub fn explain_cause<const N: usize>(
    evidence: &CausalEvidence<N>,
    effect: EvidenceIdentity,
    relationship: CausalRelationship,
) -> Result<LifecycleExplanation, CausalEvidenceRefusal> {
    Ok(LifecycleExplanation {
        summary: conduit_observatory::explain_cause(evidence, effect, relationship)?,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::causal_evidence::CausalEdge;

    #[test]
    fn patchbay_preserves_its_causal_explanation_entrance() {
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
                effect,
                relationship: CausalRelationship::CausedBy,
                cause,
            })
            .unwrap();

        assert!(
            explain_cause(&evidence, effect, CausalRelationship::CausedBy)
                .unwrap()
                .summary
                .contains("evidence 1")
        );
        let _: CausalExplanationVisibility = CausalExplanationVisibility::Public;
    }
}
