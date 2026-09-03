//! Exact PB2007-derived input boundary for the bounded Tongues research proof.

use conduit_core::{
    semantic_digest, BoundedResourceRef, KindId, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
use conduit_data::{prove_splits_disjoint, DatasetDescriptor, DatasetSplitMembership};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const PB2007_SLICE_SCHEMA: &str = "conduit.tongues/pb2007-derived-slice@1";
pub const PB2007_ARCHIVE_SHA256: &str =
    "123d3fc2f114ab37724c7f05e00a03ff21d7e815f7f957987e8255f56d73f243";
pub const PB2007_SLICE_BYTES: &[u8] = include_bytes!("../data/pb2007-derived-slice.json");
pub const TRAINING_MODALITIES: [&str; 2] = ["acoustic-observation", "articulatory-observation"];

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Pb2007Slice {
    pub schema: String,
    pub source: SourceIdentity,
    pub derivation: DerivationIdentity,
    pub resources: Vec<SourceResource>,
    pub utterances: Vec<DerivedUtterance>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceIdentity {
    pub doi: String,
    pub archive_sha256: String,
    pub archive_bytes: u64,
    pub license_deposit_readme: String,
    pub license_zenodo_metadata: String,
    pub citation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DerivationIdentity {
    pub identity: String,
    pub bins_per_utterance: usize,
    pub audio_clock_hz: u32,
    pub ema_clock_hz: u32,
    pub acoustic_features: Vec<String>,
    pub articulatory_coordinates: Vec<String>,
    pub coordinate_unit: String,
    pub head_correction: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceResource {
    pub identity: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DerivedUtterance {
    pub identity: String,
    pub split: String,
    pub speaker_context: u16,
    pub audio_sample_count: u32,
    pub ema_sample_count: u32,
    pub missing_mask: Vec<bool>,
    pub acoustic: Vec<Vec<i64>>,
    pub articulation: Vec<Vec<i64>>,
    pub post_freeze_probe_labels: Vec<ProbeSegment>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProbeSegment {
    pub start_bin: usize,
    pub end_bin: usize,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct TrainingUtterance {
    pub identity: String,
    pub split: String,
    pub speaker_context: u16,
    pub missing_mask: Vec<bool>,
    pub acoustic: Vec<Vec<i64>>,
    pub articulation: Vec<Vec<i64>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenRepresentation {
    checkpoint_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResearchDataError {
    Malformed,
    WrongIdentity,
    InvalidBounds,
    DuplicateIdentity,
    SplitLeakage,
}

impl Pb2007Slice {
    pub fn load() -> Result<Self, ResearchDataError> {
        let value: Self =
            serde_json::from_slice(PB2007_SLICE_BYTES).map_err(|_| ResearchDataError::Malformed)?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), ResearchDataError> {
        if self.schema != PB2007_SLICE_SCHEMA
            || self.source.doi != "10.5281/zenodo.6390598"
            || self.source.archive_sha256 != PB2007_ARCHIVE_SHA256
            || self.source.archive_bytes != 37_793_957
            || self.source.license_deposit_readme != "CC-BY-SA"
            || self.derivation.bins_per_utterance != 16
            || self.derivation.audio_clock_hz != 16_000
            || self.derivation.ema_clock_hz != 100
            || self.derivation.acoustic_features.len() != 4
            || self.derivation.articulatory_coordinates.len() != 6
            || self.resources.len() != 36
            || self.utterances.len() != 12
        {
            return Err(ResearchDataError::WrongIdentity);
        }
        let mut identities = BTreeSet::new();
        for resource in &self.resources {
            if resource.bytes == 0
                || resource.sha256.len() != 64
                || !identities.insert(resource.identity.as_str())
            {
                return Err(ResearchDataError::DuplicateIdentity);
            }
        }
        identities.clear();
        for utterance in &self.utterances {
            if !identities.insert(utterance.identity.as_str()) {
                return Err(ResearchDataError::DuplicateIdentity);
            }
            if !matches!(utterance.split.as_str(), "train" | "validation" | "test")
                || utterance.audio_sample_count == 0
                || utterance.ema_sample_count == 0
                || utterance.missing_mask.len() != 16
                || utterance.acoustic.len() != 16
                || utterance.articulation.len() != 16
                || utterance.acoustic.iter().any(|frame| frame.len() != 4)
                || utterance.articulation.iter().any(|frame| frame.len() != 6)
                || utterance.post_freeze_probe_labels.iter().any(|segment| {
                    segment.label.is_empty()
                        || segment.start_bin >= segment.end_bin
                        || segment.end_bin > 16
                })
            {
                return Err(ResearchDataError::InvalidBounds);
            }
        }
        let (descriptor, splits) = self.dataset_contract()?;
        descriptor
            .validate()
            .map_err(|_| ResearchDataError::Malformed)?;
        for left in 0..splits.len() {
            for right in left + 1..splits.len() {
                prove_splits_disjoint(&splits[left], &splits[right])
                    .map_err(|_| ResearchDataError::SplitLeakage)?;
            }
        }
        Ok(())
    }

    /// The representation learner receives this label-free type. Probe labels
    /// cannot cross this API before an exact checkpoint has frozen the model.
    pub fn training_utterances(&self) -> Vec<TrainingUtterance> {
        self.utterances
            .iter()
            .map(|value| TrainingUtterance {
                identity: value.identity.clone(),
                split: value.split.clone(),
                speaker_context: value.speaker_context,
                missing_mask: value.missing_mask.clone(),
                acoustic: value.acoustic.clone(),
                articulation: value.articulation.clone(),
            })
            .collect()
    }

    pub fn freeze(
        &self,
        checkpoint_identity: [u8; 32],
    ) -> Result<FrozenRepresentation, ResearchDataError> {
        if checkpoint_identity == [0; 32] {
            return Err(ResearchDataError::WrongIdentity);
        }
        Ok(FrozenRepresentation {
            checkpoint_identity,
        })
    }

    pub fn probe_labels<'a>(
        &'a self,
        frozen: &FrozenRepresentation,
    ) -> Result<Vec<(&'a str, &'a [ProbeSegment])>, ResearchDataError> {
        if frozen.checkpoint_identity == [0; 32] {
            return Err(ResearchDataError::WrongIdentity);
        }
        Ok(self
            .utterances
            .iter()
            .map(|value| {
                (
                    value.identity.as_str(),
                    value.post_freeze_probe_labels.as_slice(),
                )
            })
            .collect())
    }

    pub fn dataset_contract(
        &self,
    ) -> Result<(DatasetDescriptor, Vec<DatasetSplitMembership>), ResearchDataError> {
        let manifest_digest: [u8; 32] = Sha256::digest(PB2007_SLICE_BYTES).into();
        let dataset_identity =
            semantic_digest("conduit.tongues/pb2007-dataset@1", &manifest_digest);
        let descriptor = DatasetDescriptor {
            identity: dataset_identity,
            schema_profile: "paired-audio-ema/no-training-labels@1".into(),
            citation_identity: Some("doi:10.5281/zenodo.6390598".into()),
            license_profile: Some("CC-BY-SA (deposit README; Zenodo metadata differs)".into()),
            example_count: self.utterances.len() as u64,
            manifest: resource(
                manifest_digest,
                "data/corpus-manifest@1",
                PB2007_SLICE_BYTES.len() as u64,
            ),
            shards: self
                .utterances
                .iter()
                .map(|value| {
                    resource(
                        example_identity(&value.identity),
                        "data/paired-observation-shard@1",
                        16,
                    )
                })
                .collect(),
            split_identities: vec!["train".into(), "validation".into(), "test".into()],
        };
        let splits = descriptor
            .split_identities
            .iter()
            .map(|split| DatasetSplitMembership {
                dataset_identity,
                split_identity: split.clone(),
                examples: self
                    .utterances
                    .iter()
                    .filter(|value| &value.split == split)
                    .map(|value| example_identity(&value.identity))
                    .collect(),
            })
            .collect();
        Ok((descriptor, splits))
    }
}

pub fn example_identity(identity: &str) -> [u8; 32] {
    semantic_digest("conduit.tongues/pb2007-example@1", identity.as_bytes())
}

fn resource(identity: [u8; 32], profile: &str, bytes: u64) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(identity),
        content_profile: KindId::from(profile),
        access_class: ResourceClassId::from("scientific-corpus/read@1"),
        extent: ResourceExtent { bytes, items: None },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest(identity),
            expires_at: None,
        },
    }
}
