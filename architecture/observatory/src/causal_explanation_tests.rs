use crate::*;
use conduit_kernel::causal_evidence::{
    CausalEdge, CausalEvidence, CausalRelationship, CausalTraceCompleteness, EvidenceIdentity,
    EvidenceMetadataFact, EvidenceMetadataLookup, EvidenceMetadataVisit, EvidenceOutcome,
    TerminalEvidenceCorrelation, TerminalEvidenceIndex,
};

struct ExactMetadata(EvidenceIdentity);

impl EvidenceMetadataLookup for ExactMetadata {
    fn visit<'a>(
        &'a self,
        evidence: EvidenceIdentity,
        visitor: &mut dyn FnMut(EvidenceMetadataFact<'a>) -> bool,
    ) -> EvidenceMetadataVisit {
        if evidence != self.0 {
            return EvidenceMetadataVisit::Missing;
        }
        for fact in [
            EvidenceMetadataFact::Outcome(EvidenceOutcome::SemanticTerminal),
            EvidenceMetadataFact::Source {
                document: "source/exact",
                start: Some(4),
                end: Some(12),
                line: Some(2),
                column: Some(3),
                end_line: Some(2),
                end_column: Some(11),
            },
            EvidenceMetadataFact::Implementation("back/exact"),
            EvidenceMetadataFact::Host("host/private"),
            EvidenceMetadataFact::Authority {
                grant: "grant/private",
                contract: "authority/audio",
            },
        ] {
            if !visitor(fact) {
                return EvidenceMetadataVisit::VisitorRefused;
            }
        }
        EvidenceMetadataVisit::Visited
    }
}

#[test]
fn projects_branching_evidence_by_visibility() {
    let terminal = identity(4);
    let mut evidence = CausalEvidence::<4>::default();
    for edge in [
        CausalEdge {
            effect: terminal,
            relationship: CausalRelationship::TerminatedBecause,
            cause: identity(3),
        },
        CausalEdge {
            effect: identity(3),
            relationship: CausalRelationship::CausedBy,
            cause: identity(1),
        },
        CausalEdge {
            effect: identity(3),
            relationship: CausalRelationship::CausedBy,
            cause: identity(2),
        },
    ] {
        evidence.record(edge).unwrap();
    }

    let metadata = ExactMetadata(terminal);
    let public = explain_trace_with_metadata(
        &evidence,
        &metadata,
        terminal,
        CausalExplanationVisibility::Public,
    )
    .unwrap();
    assert_eq!(public.nodes.len(), 4);
    assert_eq!(public.edges.len(), 3);
    assert!(public.nodes.iter().all(|node| node.evidence.is_none()));
    assert_eq!(
        public.nodes[0].metadata,
        CausalExplanationMetadata::Redacted
    );

    let operator = explain_trace_with_metadata(
        &evidence,
        &metadata,
        terminal,
        CausalExplanationVisibility::Operator,
    )
    .unwrap();
    assert_eq!(operator.edges, public.edges);
    assert_eq!(operator.nodes[0].evidence, Some(terminal));
    assert_eq!(
        operator.nodes[0].outcome,
        Some(EvidenceOutcome::SemanticTerminal)
    );
    assert!(matches!(
        &operator.nodes[0].metadata,
        CausalExplanationMetadata::Visible(facts)
            if facts.contains(&CausalExplanationMetadataFact::Implementation("back/exact".into()))
                && facts.contains(&CausalExplanationMetadataFact::Authority {
                    grant: "grant/private".into(),
                    contract: "authority/audio".into(),
                })
    ));
    assert!(operator
        .nodes
        .iter()
        .skip(1)
        .all(|node| { node.metadata == CausalExplanationMetadata::Missing }));
}

#[test]
fn resolves_terminal_and_marks_compacted_history_incomplete() {
    let digest = [7; 32];
    let terminal = identity(4);
    let mut correlations = TerminalEvidenceIndex::<1>::default();
    correlations
        .record(TerminalEvidenceCorrelation {
            cause_digest: digest,
            terminal,
        })
        .unwrap();
    let mut evidence = CausalEvidence::<2>::default();
    for sign in 1..=3 {
        evidence
            .record(CausalEdge {
                effect: identity(sign + 1),
                relationship: CausalRelationship::CausedBy,
                cause: identity(sign),
            })
            .unwrap();
    }
    let explanation = explain_terminal(
        &correlations,
        &evidence,
        digest,
        CausalExplanationVisibility::Operator,
    )
    .unwrap();
    assert_eq!(
        explanation.completeness,
        CausalTraceCompleteness::IncompleteHistory
    );
}

#[test]
fn terminal_resolution_and_exact_metadata_projection_are_one_bounded_operation() {
    let digest = [8; 32];
    let terminal = identity(2);
    let mut correlations = TerminalEvidenceIndex::<1>::default();
    correlations
        .record(TerminalEvidenceCorrelation {
            cause_digest: digest,
            terminal,
        })
        .unwrap();
    let mut evidence = CausalEvidence::<1>::default();
    evidence
        .record(CausalEdge {
            effect: terminal,
            relationship: CausalRelationship::TerminatedBecause,
            cause: identity(1),
        })
        .unwrap();

    let explanation = explain_terminal_with_metadata(
        &correlations,
        &evidence,
        &ExactMetadata(terminal),
        digest,
        CausalExplanationVisibility::Operator,
    )
    .unwrap();

    assert_eq!(explanation.nodes[0].evidence, Some(terminal));
    assert_eq!(
        explanation.nodes[0].outcome,
        Some(EvidenceOutcome::SemanticTerminal)
    );
    assert!(matches!(
        &explanation.nodes[0].metadata,
        CausalExplanationMetadata::Visible(facts)
            if facts.contains(&CausalExplanationMetadataFact::Source {
                document: "source/exact".into(),
                start: Some(4),
                end: Some(12),
                line: Some(2),
                column: Some(3),
                end_line: Some(2),
                end_column: Some(11),
            })
    ));
}

#[test]
fn refuses_projection_beyond_inspection_envelope() {
    let terminal = identity(MAXIMUM_CAUSAL_EXPLANATION_EDGES as u64 + 2);
    let mut evidence = CausalEvidence::<{ MAXIMUM_CAUSAL_EXPLANATION_EDGES + 1 }>::default();
    for sign in 1..=MAXIMUM_CAUSAL_EXPLANATION_EDGES as u64 + 1 {
        evidence
            .record(CausalEdge {
                effect: identity(sign + 1),
                relationship: CausalRelationship::CausedBy,
                cause: identity(sign),
            })
            .unwrap();
    }
    assert_eq!(
        explain_trace(&evidence, terminal, CausalExplanationVisibility::Public),
        Err(CausalExplanationRefusal::InspectionEnvelopeExceeded)
    );
}

fn identity(sign: u64) -> EvidenceIdentity {
    EvidenceIdentity {
        sign,
        execution: 1,
        host_session: 1,
    }
}
