use conduit_kernel::{causal_evidence::*, fault_disposition::*, *};

fn id(sign: u64, execution: u64, session: u64) -> EvidenceIdentity {
    EvidenceIdentity {
        sign,
        execution,
        host_session: session,
    }
}

struct BorrowedMetadata;

impl EvidenceMetadataLookup for BorrowedMetadata {
    fn visit<'a>(
        &'a self,
        evidence: EvidenceIdentity,
        visitor: &mut dyn FnMut(EvidenceMetadataFact<'a>) -> bool,
    ) -> EvidenceMetadataVisit {
        if evidence != id(1, 1, 1) {
            return EvidenceMetadataVisit::Missing;
        }
        for fact in [
            EvidenceMetadataFact::Outcome(EvidenceOutcome::Recovered),
            EvidenceMetadataFact::Plan("plan/exact"),
            EvidenceMetadataFact::Host("host/exact"),
        ] {
            if !visitor(fact) {
                return EvidenceMetadataVisit::VisitorRefused;
            }
        }
        EvidenceMetadataVisit::Visited
    }
}

#[test]
fn metadata_lookup_lends_exact_identity_without_changing_graph_storage() {
    let lookup = BorrowedMetadata;
    let mut facts = std::vec::Vec::new();
    assert_eq!(
        lookup.visit(id(1, 1, 1), &mut |fact| {
            facts.push(fact);
            true
        }),
        EvidenceMetadataVisit::Visited
    );
    assert_eq!(
        facts,
        vec![
            EvidenceMetadataFact::Outcome(EvidenceOutcome::Recovered),
            EvidenceMetadataFact::Plan("plan/exact"),
            EvidenceMetadataFact::Host("host/exact"),
        ]
    );
    assert_eq!(
        lookup.visit(id(2, 1, 1), &mut |_| true),
        EvidenceMetadataVisit::Missing
    );
    assert_eq!(
        lookup.visit(id(1, 1, 1), &mut |_| false),
        EvidenceMetadataVisit::VisitorRefused
    );

    let evidence = CausalEvidence::<1>::default();
    assert!(!evidence.history_was_truncated());
}

#[test]
fn semantic_terminal_digest_resolves_to_exact_causal_evidence() {
    let digest = [7; 32];
    let terminal = id(4, 2, 2);
    let mut correlations = TerminalEvidenceIndex::<2>::default();
    correlations
        .record(TerminalEvidenceCorrelation {
            cause_digest: digest,
            terminal,
        })
        .unwrap();
    let mut evidence = CausalEvidence::<2>::default();
    evidence
        .record(CausalEdge {
            effect: terminal,
            relationship: CausalRelationship::TerminatedBecause,
            cause: id(3, 1, 1),
        })
        .unwrap();

    let resolved = correlations.terminal_for(digest).unwrap();
    assert_eq!(resolved, terminal);
    assert_eq!(evidence.trace(resolved).unwrap().terminal(), terminal);
}

#[test]
fn terminal_correlation_never_guesses_after_ambiguity_or_compaction() {
    let digest = [9; 32];
    let mut correlations = TerminalEvidenceIndex::<2>::default();
    for terminal in [id(1, 1, 1), id(2, 1, 1)] {
        correlations
            .record(TerminalEvidenceCorrelation {
                cause_digest: digest,
                terminal,
            })
            .unwrap();
    }
    assert_eq!(
        correlations.terminal_for(digest),
        Err(CausalEvidenceRefusal::AmbiguousCorrelation)
    );

    correlations
        .record(TerminalEvidenceCorrelation {
            cause_digest: [10; 32],
            terminal: id(3, 1, 1),
        })
        .unwrap();
    correlations
        .record(TerminalEvidenceCorrelation {
            cause_digest: [11; 32],
            terminal: id(4, 1, 1),
        })
        .unwrap();
    assert!(correlations.history_was_truncated());
    assert_eq!(
        correlations.terminal_for(digest),
        Err(CausalEvidenceRefusal::TruncatedHistory)
    );
    assert_eq!(correlations.terminal_for([10; 32]), Ok(id(3, 1, 1)));
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
fn oldest_edges_compact_deterministically_and_make_truncation_explicit() {
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
    evidence
        .record(CausalEdge {
            cause: id(3, 1, 1),
            relationship: CausalRelationship::Supersedes,
            effect: id(4, 1, 1),
        })
        .unwrap();
    assert!(evidence.history_was_truncated());
    let trace = evidence.trace(id(4, 1, 1)).unwrap();
    assert_eq!(
        trace.completeness(),
        CausalTraceCompleteness::IncompleteHistory
    );
    assert_eq!(trace.edges().copied().collect::<Vec<_>>().len(), 2);
    assert_eq!(
        evidence.cause_of(id(2, 1, 1), CausalRelationship::DerivedFrom),
        Err(CausalEvidenceRefusal::Unknown)
    );
}

#[test]
fn cycles_and_excessive_direct_fan_in_refuse_before_mutation() {
    let mut evidence = CausalEvidence::<16>::default();
    evidence
        .record(CausalEdge {
            effect: id(3, 1, 1),
            relationship: CausalRelationship::CausedBy,
            cause: id(2, 1, 1),
        })
        .unwrap();
    evidence
        .record(CausalEdge {
            effect: id(2, 1, 1),
            relationship: CausalRelationship::CausedBy,
            cause: id(1, 1, 1),
        })
        .unwrap();
    assert_eq!(
        evidence.record(CausalEdge {
            effect: id(1, 1, 1),
            relationship: CausalRelationship::CausedBy,
            cause: id(3, 1, 1),
        }),
        Err(CausalEvidenceRefusal::Cycle)
    );

    let effect = id(20, 1, 1);
    for predecessor in 0..MAXIMUM_DIRECT_CAUSAL_PREDECESSORS {
        evidence
            .record(CausalEdge {
                effect,
                relationship: CausalRelationship::CausedBy,
                cause: id(30 + predecessor as u64, 1, 1),
            })
            .unwrap();
    }
    assert_eq!(
        evidence.record(CausalEdge {
            effect,
            relationship: CausalRelationship::CausedBy,
            cause: id(99, 1, 1),
        }),
        Err(CausalEvidenceRefusal::TooManyDirectPredecessors)
    );
}

#[test]
fn trace_preserves_branching_and_merging_without_timestamp_inference() {
    let mut evidence = CausalEvidence::<8>::default();
    for edge in [
        CausalEdge {
            effect: id(4, 2, 2),
            relationship: CausalRelationship::TerminatedBecause,
            cause: id(3, 1, 1),
        },
        CausalEdge {
            effect: id(3, 1, 1),
            relationship: CausalRelationship::CausedBy,
            cause: id(1, 1, 1),
        },
        CausalEdge {
            effect: id(3, 1, 1),
            relationship: CausalRelationship::CausedBy,
            cause: id(2, 1, 1),
        },
    ] {
        evidence.record(edge).unwrap();
    }
    let trace = evidence.trace(id(4, 2, 2)).unwrap();
    assert_eq!(trace.terminal(), id(4, 2, 2));
    assert_eq!(trace.completeness(), CausalTraceCompleteness::Complete);
    assert_eq!(trace.edges().count(), 3);
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
