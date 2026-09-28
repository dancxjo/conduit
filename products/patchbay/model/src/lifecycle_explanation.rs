//! Read-only projection of generic lifecycle evidence.

use conduit_body::{BodyAdministrativeResult, WorkloadTransitionState};
use conduit_core::ResourceAcquisitionState;
use conduit_kernel::{
    causal_evidence::{
        CausalEvidence, CausalEvidenceRefusal, CausalRelationship, CausalTraceCompleteness,
        EvidenceIdentity, EvidenceMetadataFact, EvidenceMetadataLookup, EvidenceMetadataVisit,
        EvidenceOutcome, TerminalEvidenceIndex,
    },
    fault_disposition::FaultDisposition,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleExplanation {
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausalExplanationVisibility {
    /// Preserve the causal graph while withholding execution and Host-session
    /// correlation identities.
    Public,
    /// Reveal exact retained correlation identities to an authorized operator.
    Operator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalExplanationNode {
    pub ordinal: u16,
    pub terminal: bool,
    pub evidence: Option<EvidenceIdentity>,
    pub outcome: Option<EvidenceOutcome>,
    pub metadata: CausalExplanationMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CausalExplanationMetadata {
    Missing,
    Redacted,
    Visible(Vec<CausalExplanationMetadataFact>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CausalExplanationMetadataFact {
    SemanticSubject {
        gear: String,
        kind: String,
    },
    Source {
        document: String,
        start: Option<u32>,
        end: Option<u32>,
    },
    Wake(String),
    Plan(String),
    Play(String),
    Placement(String),
    Implementation(String),
    Host(String),
    Boot(String),
    Resource {
        pool: String,
        generation: Option<String>,
    },
    Authority {
        grant: String,
        contract: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalExplanationEdge {
    pub effect: u16,
    pub relationship: CausalRelationship,
    pub cause: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalTraceExplanation {
    pub summary: String,
    pub completeness: CausalTraceCompleteness,
    pub visibility: CausalExplanationVisibility,
    pub nodes: Vec<CausalExplanationNode>,
    pub edges: Vec<CausalExplanationEdge>,
}

pub const MAXIMUM_CAUSAL_EXPLANATION_EDGES: usize = 128;
pub const MAXIMUM_CAUSAL_EXPLANATION_NODES: usize = MAXIMUM_CAUSAL_EXPLANATION_EDGES * 2 + 1;
pub const MAXIMUM_CAUSAL_METADATA_FACTS_PER_NODE: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausalExplanationRefusal {
    Evidence(CausalEvidenceRefusal),
    InspectionEnvelopeExceeded,
}

impl From<CausalEvidenceRefusal> for CausalExplanationRefusal {
    fn from(value: CausalEvidenceRefusal) -> Self {
        Self::Evidence(value)
    }
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

/// Resolves the correlation carried by one semantic abnormal terminal and
/// projects its retained causal DAG without converting it into a fake stack.
pub fn explain_terminal<const CORRELATIONS: usize, const EDGES: usize>(
    correlations: &TerminalEvidenceIndex<CORRELATIONS>,
    evidence: &CausalEvidence<EDGES>,
    cause_digest: [u8; 32],
    visibility: CausalExplanationVisibility,
) -> Result<CausalTraceExplanation, CausalExplanationRefusal> {
    explain_trace(
        evidence,
        correlations.terminal_for(cause_digest)?,
        visibility,
    )
}

pub fn explain_trace<const N: usize>(
    evidence: &CausalEvidence<N>,
    terminal: EvidenceIdentity,
    visibility: CausalExplanationVisibility,
) -> Result<CausalTraceExplanation, CausalExplanationRefusal> {
    struct NoMetadata;
    impl EvidenceMetadataLookup for NoMetadata {
        fn visit<'a>(
            &'a self,
            _evidence: EvidenceIdentity,
            _visitor: &mut dyn FnMut(EvidenceMetadataFact<'a>) -> bool,
        ) -> EvidenceMetadataVisit {
            EvidenceMetadataVisit::Missing
        }
    }
    explain_trace_with_metadata(evidence, &NoMetadata, terminal, visibility)
}

pub fn explain_trace_with_metadata<const N: usize>(
    evidence: &CausalEvidence<N>,
    metadata: &impl EvidenceMetadataLookup,
    terminal: EvidenceIdentity,
    visibility: CausalExplanationVisibility,
) -> Result<CausalTraceExplanation, CausalExplanationRefusal> {
    let trace = evidence.trace(terminal)?;
    let edge_count = trace.edges().count();
    if edge_count > MAXIMUM_CAUSAL_EXPLANATION_EDGES {
        return Err(CausalExplanationRefusal::InspectionEnvelopeExceeded);
    }
    let mut identities = Vec::with_capacity(edge_count.saturating_mul(2) + 1);
    identities.push(terminal);
    for edge in trace.edges() {
        for identity in [edge.effect, edge.cause] {
            if !identities.contains(&identity) {
                identities.push(identity);
            }
        }
    }
    if identities.len() > MAXIMUM_CAUSAL_EXPLANATION_NODES {
        return Err(CausalExplanationRefusal::InspectionEnvelopeExceeded);
    }
    let nodes = identities
        .iter()
        .enumerate()
        .map(|(index, identity)| {
            let (outcome, projected) = project_metadata(metadata, *identity, visibility)?;
            Ok(CausalExplanationNode {
                ordinal: u16::try_from(index)
                    .expect("causal evidence capacity fits one u16 ordinal"),
                terminal: identity == &terminal,
                evidence: (visibility == CausalExplanationVisibility::Operator)
                    .then_some(*identity),
                outcome,
                metadata: projected,
            })
        })
        .collect::<Result<Vec<_>, CausalExplanationRefusal>>()?;
    let edges = trace
        .edges()
        .map(|edge| CausalExplanationEdge {
            effect: ordinal(&identities, edge.effect),
            relationship: edge.relationship,
            cause: ordinal(&identities, edge.cause),
        })
        .collect::<Vec<_>>();
    let completeness = trace.completeness();
    Ok(CausalTraceExplanation {
        summary: format!(
            "{} retained causal relationship{} ({})",
            edges.len(),
            if edges.len() == 1 { "" } else { "s" },
            match completeness {
                CausalTraceCompleteness::Complete => "complete retained history",
                CausalTraceCompleteness::IncompleteHistory => "incomplete retained history",
            }
        ),
        completeness,
        visibility,
        nodes,
        edges,
    })
}

fn project_metadata(
    lookup: &impl EvidenceMetadataLookup,
    identity: EvidenceIdentity,
    visibility: CausalExplanationVisibility,
) -> Result<(Option<EvidenceOutcome>, CausalExplanationMetadata), CausalExplanationRefusal> {
    let mut outcome = None;
    let mut exact = Vec::new();
    let mut redacted = false;
    let visit = lookup.visit(identity, &mut |fact| {
        if let EvidenceMetadataFact::Outcome(value) = fact {
            outcome = Some(value);
            return true;
        }
        if visibility == CausalExplanationVisibility::Public {
            redacted = true;
            return true;
        }
        if exact.len() == MAXIMUM_CAUSAL_METADATA_FACTS_PER_NODE {
            return false;
        }
        exact.push(owned_metadata_fact(fact));
        true
    });
    match visit {
        EvidenceMetadataVisit::Missing => Ok((outcome, CausalExplanationMetadata::Missing)),
        EvidenceMetadataVisit::VisitorRefused => {
            Err(CausalExplanationRefusal::InspectionEnvelopeExceeded)
        }
        EvidenceMetadataVisit::Visited if visibility == CausalExplanationVisibility::Public => {
            Ok((
                outcome,
                if redacted {
                    CausalExplanationMetadata::Redacted
                } else {
                    CausalExplanationMetadata::Visible(Vec::new())
                },
            ))
        }
        EvidenceMetadataVisit::Visited => Ok((outcome, CausalExplanationMetadata::Visible(exact))),
    }
}

fn owned_metadata_fact(fact: EvidenceMetadataFact<'_>) -> CausalExplanationMetadataFact {
    match fact {
        EvidenceMetadataFact::Outcome(_) => unreachable!("outcome is projected separately"),
        EvidenceMetadataFact::SemanticSubject { gear, kind } => {
            CausalExplanationMetadataFact::SemanticSubject {
                gear: gear.into(),
                kind: kind.into(),
            }
        }
        EvidenceMetadataFact::Source {
            document,
            start,
            end,
        } => CausalExplanationMetadataFact::Source {
            document: document.into(),
            start,
            end,
        },
        EvidenceMetadataFact::Wake(value) => CausalExplanationMetadataFact::Wake(value.into()),
        EvidenceMetadataFact::Plan(value) => CausalExplanationMetadataFact::Plan(value.into()),
        EvidenceMetadataFact::Play(value) => CausalExplanationMetadataFact::Play(value.into()),
        EvidenceMetadataFact::Placement(value) => {
            CausalExplanationMetadataFact::Placement(value.into())
        }
        EvidenceMetadataFact::Implementation(value) => {
            CausalExplanationMetadataFact::Implementation(value.into())
        }
        EvidenceMetadataFact::Host(value) => CausalExplanationMetadataFact::Host(value.into()),
        EvidenceMetadataFact::Boot(value) => CausalExplanationMetadataFact::Boot(value.into()),
        EvidenceMetadataFact::Resource { pool, generation } => {
            CausalExplanationMetadataFact::Resource {
                pool: pool.into(),
                generation: generation.map(Into::into),
            }
        }
        EvidenceMetadataFact::Authority { grant, contract } => {
            CausalExplanationMetadataFact::Authority {
                grant: grant.into(),
                contract: contract.into(),
            }
        }
    }
}

fn ordinal(identities: &[EvidenceIdentity], identity: EvidenceIdentity) -> u16 {
    let index = identities
        .iter()
        .position(|candidate| candidate == &identity)
        .expect("every causal edge identity was indexed");
    u16::try_from(index).expect("causal evidence capacity fits one u16 ordinal")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::causal_evidence::{CausalEdge, CausalRelationship};

    struct ExactMetadata {
        terminal: EvidenceIdentity,
    }

    impl EvidenceMetadataLookup for ExactMetadata {
        fn visit<'a>(
            &'a self,
            evidence: EvidenceIdentity,
            visitor: &mut dyn FnMut(EvidenceMetadataFact<'a>) -> bool,
        ) -> EvidenceMetadataVisit {
            if evidence != self.terminal {
                return EvidenceMetadataVisit::Missing;
            }
            for fact in [
                EvidenceMetadataFact::Outcome(EvidenceOutcome::SemanticTerminal),
                EvidenceMetadataFact::Plan("plan/exact"),
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

    #[test]
    fn public_explanation_preserves_graph_shape_without_leaking_exact_identities() {
        let terminal = EvidenceIdentity {
            sign: 4,
            execution: 2,
            host_session: 2,
        };
        let mut evidence = CausalEvidence::<4>::default();
        for edge in [
            CausalEdge {
                effect: terminal,
                relationship: CausalRelationship::TerminatedBecause,
                cause: EvidenceIdentity {
                    sign: 3,
                    execution: 1,
                    host_session: 1,
                },
            },
            CausalEdge {
                effect: EvidenceIdentity {
                    sign: 3,
                    execution: 1,
                    host_session: 1,
                },
                relationship: CausalRelationship::CausedBy,
                cause: EvidenceIdentity {
                    sign: 1,
                    execution: 1,
                    host_session: 1,
                },
            },
            CausalEdge {
                effect: EvidenceIdentity {
                    sign: 3,
                    execution: 1,
                    host_session: 1,
                },
                relationship: CausalRelationship::CausedBy,
                cause: EvidenceIdentity {
                    sign: 2,
                    execution: 1,
                    host_session: 1,
                },
            },
        ] {
            evidence.record(edge).unwrap();
        }

        let metadata = ExactMetadata { terminal };
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
        assert!(public.nodes.iter().any(|node| node.terminal));
        assert_eq!(
            public.nodes[0].outcome,
            Some(EvidenceOutcome::SemanticTerminal)
        );
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
        assert!(operator.nodes.iter().all(|node| node.evidence.is_some()));
        assert_eq!(operator.nodes[0].outcome, public.nodes[0].outcome);
        assert!(matches!(
            &operator.nodes[0].metadata,
            CausalExplanationMetadata::Visible(facts)
                if facts.contains(&CausalExplanationMetadataFact::Implementation(
                    "back/exact".into()
                )) && facts.contains(&CausalExplanationMetadataFact::Authority {
                    grant: "grant/private".into(),
                    contract: "authority/audio".into(),
                })
        ));
    }

    #[test]
    fn semantic_terminal_resolution_keeps_incomplete_history_explicit() {
        let digest = [7; 32];
        let terminal = EvidenceIdentity {
            sign: 4,
            execution: 1,
            host_session: 1,
        };
        let mut correlations = TerminalEvidenceIndex::<1>::default();
        correlations
            .record(
                conduit_kernel::causal_evidence::TerminalEvidenceCorrelation {
                    cause_digest: digest,
                    terminal,
                },
            )
            .unwrap();
        let mut evidence = CausalEvidence::<2>::default();
        for sign in 1..=3 {
            evidence
                .record(CausalEdge {
                    effect: EvidenceIdentity {
                        sign: sign + 1,
                        execution: 1,
                        host_session: 1,
                    },
                    relationship: CausalRelationship::CausedBy,
                    cause: EvidenceIdentity {
                        sign,
                        execution: 1,
                        host_session: 1,
                    },
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
        assert!(explanation.summary.contains("incomplete retained history"));
    }

    #[test]
    fn patchbay_refuses_a_trace_beyond_its_own_inspection_envelope() {
        let terminal = EvidenceIdentity {
            sign: MAXIMUM_CAUSAL_EXPLANATION_EDGES as u64 + 2,
            execution: 1,
            host_session: 1,
        };
        let mut evidence = CausalEvidence::<{ MAXIMUM_CAUSAL_EXPLANATION_EDGES + 1 }>::default();
        for sign in 1..=MAXIMUM_CAUSAL_EXPLANATION_EDGES as u64 + 1 {
            evidence
                .record(CausalEdge {
                    effect: EvidenceIdentity {
                        sign: sign + 1,
                        execution: 1,
                        host_session: 1,
                    },
                    relationship: CausalRelationship::CausedBy,
                    cause: EvidenceIdentity {
                        sign,
                        execution: 1,
                        host_session: 1,
                    },
                })
                .unwrap();
        }
        assert_eq!(
            explain_trace(&evidence, terminal, CausalExplanationVisibility::Public),
            Err(CausalExplanationRefusal::InspectionEnvelopeExceeded)
        );
    }
}
