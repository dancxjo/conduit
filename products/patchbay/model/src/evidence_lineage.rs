//! Renderer-neutral Patchbay projection of exact derived-evidence lineage.

use conduit_observatory::{EvidenceLineageRefusal, EvidenceLineageReport, EvidenceLineageStage};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayEvidenceLineageRow {
    pub identity: String,
    pub stage: EvidenceLineageStage,
    pub causal_inputs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PatchbayEvidenceLineage {
    pub report_identity: String,
    pub rows: Vec<PatchbayEvidenceLineageRow>,
}

impl PatchbayEvidenceLineage {
    pub fn project(report: &EvidenceLineageReport) -> Result<Self, EvidenceLineageRefusal> {
        report.validate()?;
        let mut rows = report
            .nodes
            .iter()
            .map(|node| {
                let mut causal_inputs: Vec<_> = report
                    .edges
                    .iter()
                    .filter(|edge| edge.to == node.identity)
                    .map(|edge| edge.from.clone())
                    .collect();
                causal_inputs.sort();
                PatchbayEvidenceLineageRow {
                    identity: node.identity.clone(),
                    stage: node.stage,
                    causal_inputs,
                }
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            left.stage
                .cmp(&right.stage)
                .then_with(|| left.identity.cmp(&right.identity))
        });
        Ok(Self {
            report_identity: report.identity.clone(),
            rows,
        })
    }

    pub fn row(&self, identity: &str) -> Option<&PatchbayEvidenceLineageRow> {
        self.rows.iter().find(|row| row.identity == identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_observatory::{EvidenceLineageEdge, EvidenceLineageNode};

    #[test]
    fn projection_keeps_candidates_selection_sources_and_inference_separate() {
        let report = EvidenceLineageReport::new(
            "recollection/request/1".into(),
            vec![
                EvidenceLineageNode {
                    identity: "original/sign/1".into(),
                    stage: EvidenceLineageStage::OriginalFact,
                },
                EvidenceLineageNode {
                    identity: "candidate/experience/1".into(),
                    stage: EvidenceLineageStage::RetrievalCandidate,
                },
                EvidenceLineageNode {
                    identity: "selected/experience/1".into(),
                    stage: EvidenceLineageStage::SelectedEvidence,
                },
                EvidenceLineageNode {
                    identity: "model/run/1".into(),
                    stage: EvidenceLineageStage::ModelInference,
                },
            ],
            vec![
                EvidenceLineageEdge {
                    from: "original/sign/1".into(),
                    to: "candidate/experience/1".into(),
                },
                EvidenceLineageEdge {
                    from: "candidate/experience/1".into(),
                    to: "selected/experience/1".into(),
                },
                EvidenceLineageEdge {
                    from: "selected/experience/1".into(),
                    to: "model/run/1".into(),
                },
            ],
        )
        .unwrap();
        let projection = PatchbayEvidenceLineage::project(&report).unwrap();
        assert_eq!(
            projection.row("model/run/1").unwrap().causal_inputs,
            ["selected/experience/1"]
        );
        assert_eq!(
            projection
                .row("selected/experience/1")
                .unwrap()
                .causal_inputs,
            ["candidate/experience/1"]
        );
        assert_eq!(
            projection
                .row("candidate/experience/1")
                .unwrap()
                .causal_inputs,
            ["original/sign/1"]
        );
    }
}
