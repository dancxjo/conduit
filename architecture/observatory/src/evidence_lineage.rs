//! Bounded renderer-neutral lineage for derived evidence.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_EVIDENCE_LINEAGE_NODES: usize = 128;
pub const MAXIMUM_EVIDENCE_LINEAGE_EDGES: usize = 256;
pub const MAXIMUM_EVIDENCE_LINEAGE_IDENTITY_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum EvidenceLineageStage {
    OriginalFact,
    RetrievalCandidate,
    SelectedEvidence,
    ModelInference,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceLineageNode {
    pub identity: String,
    pub stage: EvidenceLineageStage,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceLineageEdge {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceLineageReport {
    pub identity: String,
    pub nodes: Vec<EvidenceLineageNode>,
    pub edges: Vec<EvidenceLineageEdge>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceLineageRefusal {
    InvalidIdentity,
    EmptyNodes,
    TooManyNodes,
    TooManyEdges,
    DuplicateNode,
    DuplicateEdge,
    UnknownEndpoint,
    NonCausalEdge,
}

impl EvidenceLineageReport {
    pub fn new(
        identity: String,
        nodes: Vec<EvidenceLineageNode>,
        edges: Vec<EvidenceLineageEdge>,
    ) -> Result<Self, EvidenceLineageRefusal> {
        let report = Self {
            identity,
            nodes,
            edges,
        };
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<(), EvidenceLineageRefusal> {
        validate_identity(&self.identity)?;
        if self.nodes.is_empty() {
            return Err(EvidenceLineageRefusal::EmptyNodes);
        }
        if self.nodes.len() > MAXIMUM_EVIDENCE_LINEAGE_NODES {
            return Err(EvidenceLineageRefusal::TooManyNodes);
        }
        if self.edges.len() > MAXIMUM_EVIDENCE_LINEAGE_EDGES {
            return Err(EvidenceLineageRefusal::TooManyEdges);
        }
        for (index, node) in self.nodes.iter().enumerate() {
            validate_identity(&node.identity)?;
            if self.nodes[index + 1..]
                .iter()
                .any(|other| other.identity == node.identity)
            {
                return Err(EvidenceLineageRefusal::DuplicateNode);
            }
        }
        for (index, edge) in self.edges.iter().enumerate() {
            if self.edges[index + 1..].iter().any(|other| other == edge) {
                return Err(EvidenceLineageRefusal::DuplicateEdge);
            }
            let from = self
                .nodes
                .iter()
                .find(|node| node.identity == edge.from)
                .ok_or(EvidenceLineageRefusal::UnknownEndpoint)?;
            let to = self
                .nodes
                .iter()
                .find(|node| node.identity == edge.to)
                .ok_or(EvidenceLineageRefusal::UnknownEndpoint)?;
            if from.stage >= to.stage {
                return Err(EvidenceLineageRefusal::NonCausalEdge);
            }
        }
        Ok(())
    }
}

fn validate_identity(identity: &str) -> Result<(), EvidenceLineageRefusal> {
    if identity.is_empty() || identity.len() > MAXIMUM_EVIDENCE_LINEAGE_IDENTITY_BYTES {
        Err(EvidenceLineageRefusal::InvalidIdentity)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn exact_forward_lineage_accepts_and_cycles_or_unknown_sources_refuse() {
        let nodes = vec![
            EvidenceLineageNode {
                identity: "fact/1".into(),
                stage: EvidenceLineageStage::OriginalFact,
            },
            EvidenceLineageNode {
                identity: "candidate/1".into(),
                stage: EvidenceLineageStage::RetrievalCandidate,
            },
            EvidenceLineageNode {
                identity: "model/1".into(),
                stage: EvidenceLineageStage::ModelInference,
            },
        ];
        assert!(EvidenceLineageReport::new(
            "trace/1".into(),
            nodes.clone(),
            vec![
                EvidenceLineageEdge {
                    from: "fact/1".into(),
                    to: "candidate/1".into(),
                },
                EvidenceLineageEdge {
                    from: "candidate/1".into(),
                    to: "model/1".into(),
                },
            ],
        )
        .is_ok());
        assert_eq!(
            EvidenceLineageReport::new(
                "trace/1".into(),
                nodes.clone(),
                vec![EvidenceLineageEdge {
                    from: "model/1".into(),
                    to: "fact/1".into(),
                }],
            ),
            Err(EvidenceLineageRefusal::NonCausalEdge)
        );
        assert_eq!(
            EvidenceLineageReport::new(
                "trace/1".into(),
                nodes,
                vec![EvidenceLineageEdge {
                    from: "missing".into(),
                    to: "model/1".into(),
                }],
            ),
            Err(EvidenceLineageRefusal::UnknownEndpoint)
        );
    }
}
