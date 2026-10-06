//! Bounded read-only projection of kernel causal evidence.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use conduit_kernel::causal_evidence::{
    CausalEvidence, CausalEvidenceRefusal, CausalRelationship, CausalTraceCompleteness,
    ClockCapture, ClockScale, ClockSourceMetadata, EvidenceIdentity, EvidenceMetadataFact,
    EvidenceMetadataLookup, EvidenceMetadataVisit, EvidenceOutcome, TerminalEvidenceIndex,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausalExplanationVisibility {
    /// Preserve the causal graph while withholding execution and Host-session
    /// correlation identities.
    Public,
    /// Reveal exact retained correlation identities to an authorized operator.
    /// Authorization remains the caller's responsibility.
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
        start: Option<u64>,
        end: Option<u64>,
        line: Option<u64>,
        column: Option<u64>,
        end_line: Option<u64>,
        end_column: Option<u64>,
    },
    Wake(String),
    Plan(String),
    Play(String),
    Placement(String),
    Implementation(String),
    Host(String),
    Boot(String),
    ClockObservation {
        capture: ClockCapture,
        local_ticks: u64,
        local_scale: ClockScale,
        local_basis: String,
        body: Option<BodyTimeExplanation>,
    },
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
pub struct BodyTimeExplanation {
    pub basis: String,
    pub generation: u64,
    pub correlation_age_ticks: u64,
    pub correlation_age_scale: ClockScale,
    pub earliest_ticks: u64,
    pub center_ticks: u64,
    pub latest_ticks: u64,
    pub scale: ClockScale,
    pub source: ClockSourceExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClockSourceExplanation {
    Peer {
        host: String,
        boot: String,
        policy: String,
    },
    External {
        provider: String,
        policy: String,
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

pub fn explain_cause<const N: usize>(
    evidence: &CausalEvidence<N>,
    effect: EvidenceIdentity,
    relationship: CausalRelationship,
) -> Result<String, CausalEvidenceRefusal> {
    let cause = evidence.cause_of(effect, relationship)?;
    Ok(format!(
        "evidence {} {relationship:?} evidence {}",
        effect.sign, cause.sign
    ))
}

/// Resolves the compact correlation carried by a semantic abnormal terminal
/// and projects its retained causal DAG without converting it into a stack.
pub fn explain_terminal<const CORRELATIONS: usize, const EDGES: usize>(
    correlations: &TerminalEvidenceIndex<CORRELATIONS>,
    evidence: &CausalEvidence<EDGES>,
    cause_digest: [u8; 32],
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
    explain_terminal_with_metadata(
        correlations,
        evidence,
        &NoMetadata,
        cause_digest,
        visibility,
    )
}

/// Resolves a compact semantic-terminal correlation and projects the exact
/// retained metadata owned by the execution layer in one bounded operation.
///
/// The lookup lends inspection facts only. It cannot grant authority or alter
/// the causal graph, and public projection redacts those exact identities while
/// preserving graph shape and semantic outcome.
pub fn explain_terminal_with_metadata<const CORRELATIONS: usize, const EDGES: usize>(
    correlations: &TerminalEvidenceIndex<CORRELATIONS>,
    evidence: &CausalEvidence<EDGES>,
    metadata: &impl EvidenceMetadataLookup,
    cause_digest: [u8; 32],
    visibility: CausalExplanationVisibility,
) -> Result<CausalTraceExplanation, CausalExplanationRefusal> {
    explain_trace_with_metadata(
        evidence,
        metadata,
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
            line,
            column,
            end_line,
            end_column,
        } => CausalExplanationMetadataFact::Source {
            document: document.into(),
            start,
            end,
            line,
            column,
            end_line,
            end_column,
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
        EvidenceMetadataFact::ClockObservation {
            capture,
            local_ticks,
            local_scale,
            local_basis,
            body,
        } => CausalExplanationMetadataFact::ClockObservation {
            capture,
            local_ticks,
            local_scale,
            local_basis: local_basis.into(),
            body: body.map(|body| BodyTimeExplanation {
                basis: body.basis.into(),
                generation: body.generation,
                correlation_age_ticks: body.correlation_age_ticks,
                correlation_age_scale: body.correlation_age_scale,
                earliest_ticks: body.earliest_ticks,
                center_ticks: body.center_ticks,
                latest_ticks: body.latest_ticks,
                scale: body.scale,
                source: match body.source {
                    ClockSourceMetadata::Peer { host, boot, policy } => {
                        ClockSourceExplanation::Peer {
                            host: host.into(),
                            boot: boot.into(),
                            policy: policy.into(),
                        }
                    }
                    ClockSourceMetadata::External { provider, policy } => {
                        ClockSourceExplanation::External {
                            provider: provider.into(),
                            policy: policy.into(),
                        }
                    }
                },
            }),
        },
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
