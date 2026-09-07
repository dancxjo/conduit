use conduit_kernel::{causal_evidence::*, fault_disposition::*, *};

fn id(sign: u64, execution: u64, session: u64) -> EvidenceIdentity {
    EvidenceIdentity {
        sign,
        execution,
        host_session: session,
    }
}

#[test]
fn local_and_distributed_causal_chains_are_exact_not_temporal_guesses() {
    let mut evidence = CausalEvidence::<8>::default();
    evidence
        .record(CausalEdge {
            cause: id(1, 7, 1),
            relationship: CausalRelationship::CausedBy,
            effect: id(2, 7, 1),
        })
        .unwrap();
    evidence
        .record(CausalEdge {
            cause: id(2, 7, 1),
            relationship: CausalRelationship::ObservedFrom,
            effect: id(3, 8, 2),
        })
        .unwrap();
    assert_eq!(
        evidence.cause_of(id(3, 8, 2), CausalRelationship::ObservedFrom),
        Ok(id(2, 7, 1))
    );
    assert_eq!(
        evidence.cause_of(id(3, 8, 2), CausalRelationship::CausedBy),
        Err(CausalEvidenceRefusal::Unknown)
    );
}

#[test]
fn corrections_append_and_capacity_is_finite() {
    let mut evidence = CausalEvidence::<2>::default();
    evidence
        .record(CausalEdge {
            cause: id(1, 1, 1),
            relationship: CausalRelationship::DerivedFrom,
            effect: id(2, 1, 1),
        })
        .unwrap();
    evidence
        .record(CausalEdge {
            cause: id(2, 1, 1),
            relationship: CausalRelationship::Corrects,
            effect: id(3, 1, 1),
        })
        .unwrap();
    assert_eq!(
        evidence.record(CausalEdge {
            cause: id(3, 1, 1),
            relationship: CausalRelationship::Supersedes,
            effect: id(4, 1, 1)
        }),
        Err(CausalEvidenceRefusal::CapacityExhausted)
    );
}

#[test]
fn fault_scope_isolated_degraded_replaced_and_stale_completion_refused() {
    let isolated = FaultDisposition {
        scope: FailureScope::Form(1),
        policy: PlannedFaultDisposition::TerminateScope,
    }
    .admit()
    .unwrap();
    assert_eq!(
        isolated.accepts_completion(FailureScope::Form(1)),
        Err(FaultDispositionRefusal::StaleCompletion)
    );
    assert_eq!(isolated.accepts_completion(FailureScope::Form(2)), Ok(()));
    assert!(FaultDisposition {
        scope: FailureScope::Gear(NodeId(1)),
        policy: PlannedFaultDisposition::Degrade {
            alternative: NodeId(2)
        }
    }
    .admit()
    .is_ok());
    assert!(FaultDisposition {
        scope: FailureScope::Line(RemoteEndpointId(1)),
        policy: PlannedFaultDisposition::ReplaceRealization
    }
    .admit()
    .is_ok());
    assert_eq!(
        FaultDisposition {
            scope: FailureScope::Resource(ResourceId(1)),
            policy: PlannedFaultDisposition::WaitForChange { maximum_events: 0 }
        }
        .admit(),
        Err(FaultDispositionRefusal::InvalidBound)
    );
}
