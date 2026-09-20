//! Bounded correlation contract for the portable Home journey's manifestations.

use alloc::{string::String, vec::Vec};

use crate::JOURNEY_STEP_IDS;

pub const HOME_EVIDENCE_INDEX_SCHEMA: &str = "conduit.home/cross-front-evidence-index@1";
pub const MAX_HOME_MANIFESTATIONS: usize = 6;
pub const MAX_ARTIFACTS_PER_MANIFESTATION: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HomeEvidenceRefusal {
    NoManifestations,
    TooManyManifestations,
    TooManyArtifacts,
    MissingIdentity,
    JourneyMismatch,
    DuplicateManifestation,
    DuplicateReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomeManifestationEvidence {
    pub manifestation_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub plan_id: String,
    pub play_id: String,
    pub renderer_id: String,
    pub proof_class: String,
    pub receipt_id: String,
    pub artifact_ids: Vec<String>,
    pub journey_step_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomeCrossFaceEvidenceIndex {
    pub schema: &'static str,
    pub manifestations: Vec<HomeManifestationEvidence>,
}

impl HomeCrossFaceEvidenceIndex {
    pub fn new(
        manifestations: Vec<HomeManifestationEvidence>,
    ) -> Result<Self, HomeEvidenceRefusal> {
        let index = Self {
            schema: HOME_EVIDENCE_INDEX_SCHEMA,
            manifestations,
        };
        index.validate()?;
        Ok(index)
    }

    pub fn validate(&self) -> Result<(), HomeEvidenceRefusal> {
        if self.manifestations.is_empty() {
            return Err(HomeEvidenceRefusal::NoManifestations);
        }
        if self.manifestations.len() > MAX_HOME_MANIFESTATIONS {
            return Err(HomeEvidenceRefusal::TooManyManifestations);
        }
        for (position, evidence) in self.manifestations.iter().enumerate() {
            if evidence.artifact_ids.len() > MAX_ARTIFACTS_PER_MANIFESTATION {
                return Err(HomeEvidenceRefusal::TooManyArtifacts);
            }
            if [
                evidence.manifestation_id.as_str(),
                evidence.host_id.as_str(),
                evidence.boot_id.as_str(),
                evidence.plan_id.as_str(),
                evidence.play_id.as_str(),
                evidence.renderer_id.as_str(),
                evidence.proof_class.as_str(),
                evidence.receipt_id.as_str(),
            ]
            .into_iter()
            .any(str::is_empty)
                || evidence.artifact_ids.iter().any(String::is_empty)
            {
                return Err(HomeEvidenceRefusal::MissingIdentity);
            }
            if evidence.journey_step_ids.len() != JOURNEY_STEP_IDS.len()
                || evidence
                    .journey_step_ids
                    .iter()
                    .map(String::as_str)
                    .ne(JOURNEY_STEP_IDS)
            {
                return Err(HomeEvidenceRefusal::JourneyMismatch);
            }
            for previous in &self.manifestations[..position] {
                if previous.manifestation_id == evidence.manifestation_id {
                    return Err(HomeEvidenceRefusal::DuplicateManifestation);
                }
                if previous.receipt_id == evidence.receipt_id {
                    return Err(HomeEvidenceRefusal::DuplicateReceipt);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, vec};

    fn evidence(front: &str) -> HomeManifestationEvidence {
        HomeManifestationEvidence {
            manifestation_id: format!("home/{front}"),
            host_id: format!("host/{front}"),
            boot_id: format!("boot/{front}"),
            plan_id: format!("plan/{front}"),
            play_id: format!("play/{front}"),
            renderer_id: format!("renderer/{front}"),
            proof_class: format!("proof/{front}"),
            receipt_id: format!("receipt/{front}"),
            artifact_ids: vec![format!("artifact/{front}/manifest")],
            journey_step_ids: JOURNEY_STEP_IDS.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn correlates_shared_steps_without_collapsing_front_evidence() {
        let fronts = [
            "conduitos",
            "linux-native",
            "windows-native",
            "chromium",
            "firefox",
            "semantic-voice",
        ];
        let index =
            HomeCrossFaceEvidenceIndex::new(fronts.into_iter().map(evidence).collect()).unwrap();

        assert_eq!(index.schema, HOME_EVIDENCE_INDEX_SCHEMA);
        assert_eq!(index.manifestations.len(), MAX_HOME_MANIFESTATIONS);
        assert!(
            index
                .manifestations
                .iter()
                .all(|entry| entry.journey_step_ids.len() == JOURNEY_STEP_IDS.len())
        );
    }

    #[test]
    fn refuses_drift_and_reused_receipts() {
        let mut drifted = evidence("firefox");
        drifted.journey_step_ids.swap(0, 1);
        assert_eq!(
            HomeCrossFaceEvidenceIndex::new(vec![drifted]),
            Err(HomeEvidenceRefusal::JourneyMismatch)
        );

        let chromium = evidence("chromium");
        let mut voice = evidence("voice");
        voice.receipt_id = chromium.receipt_id.clone();
        assert_eq!(
            HomeCrossFaceEvidenceIndex::new(vec![chromium, voice]),
            Err(HomeEvidenceRefusal::DuplicateReceipt)
        );
    }
}
