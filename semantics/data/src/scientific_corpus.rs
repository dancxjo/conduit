//! Large corpus identity, finite manifests, and stable split membership.

use alloc::{string::String, vec::Vec};
use conduit_core::BoundedResourceRef;

use crate::{duplicate, nonzero, text, ScientificObservationRefusal};

pub const CORPUS_MANIFEST_PROFILE: &str = "data/corpus-manifest@1";
pub const MAXIMUM_CORPUS_SHARDS: usize = 64;
pub const MAXIMUM_DATASET_SPLITS: usize = 16;
pub const MAXIMUM_SPLIT_MEMBERS_PER_RECORD: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetDescriptor {
    pub identity: [u8; 32],
    pub schema_profile: String,
    pub citation_identity: Option<String>,
    pub license_profile: Option<String>,
    pub example_count: u64,
    pub manifest: BoundedResourceRef,
    pub shards: Vec<BoundedResourceRef>,
    pub split_identities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetSplitMembership {
    pub dataset_identity: [u8; 32],
    pub split_identity: String,
    pub examples: Vec<[u8; 32]>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScientificCorpusRefusal {
    Observation(ScientificObservationRefusal),
    InvalidManifest,
    MissingManifest,
    EmptyCorpus,
    TooManyShards,
    DuplicateShard,
    InvalidSplit,
    TooManySplits,
    DuplicateSplit,
    EmptyMembership,
    TooManyMembers,
    DuplicateMember,
    DatasetMismatch,
    UnknownSplit,
    SplitLeakage,
    MissingResource,
}

impl DatasetDescriptor {
    pub fn validate(&self) -> Result<(), ScientificCorpusRefusal> {
        nonzero(self.identity).map_err(ScientificCorpusRefusal::Observation)?;
        text(&self.schema_profile).map_err(ScientificCorpusRefusal::Observation)?;
        if let Some(citation) = &self.citation_identity {
            text(citation).map_err(ScientificCorpusRefusal::Observation)?;
        }
        if let Some(license) = &self.license_profile {
            text(license).map_err(ScientificCorpusRefusal::Observation)?;
        }
        if self.example_count == 0 {
            return Err(ScientificCorpusRefusal::EmptyCorpus);
        }
        self.manifest
            .validate()
            .map_err(|_| ScientificCorpusRefusal::InvalidManifest)?;
        if self.manifest.content_profile.as_str() != CORPUS_MANIFEST_PROFILE
            || self.manifest.extent.bytes == 0
        {
            return Err(ScientificCorpusRefusal::InvalidManifest);
        }
        if self.shards.is_empty() {
            return Err(ScientificCorpusRefusal::MissingManifest);
        }
        if self.shards.len() > MAXIMUM_CORPUS_SHARDS {
            return Err(ScientificCorpusRefusal::TooManyShards);
        }
        for shard in &self.shards {
            shard
                .validate()
                .map_err(|_| ScientificCorpusRefusal::InvalidManifest)?;
            if shard.extent.bytes == 0 {
                return Err(ScientificCorpusRefusal::InvalidManifest);
            }
        }
        if duplicate(self.shards.iter().map(|shard| shard.identity.digest())) {
            return Err(ScientificCorpusRefusal::DuplicateShard);
        }
        if self.split_identities.is_empty() {
            return Err(ScientificCorpusRefusal::InvalidSplit);
        }
        if self.split_identities.len() > MAXIMUM_DATASET_SPLITS {
            return Err(ScientificCorpusRefusal::TooManySplits);
        }
        for split in &self.split_identities {
            text(split).map_err(ScientificCorpusRefusal::Observation)?;
        }
        if self
            .split_identities
            .iter()
            .enumerate()
            .any(|(index, split)| self.split_identities[index + 1..].contains(split))
        {
            return Err(ScientificCorpusRefusal::DuplicateSplit);
        }
        Ok(())
    }

    pub fn require_resources(&self, available: &[[u8; 32]]) -> Result<(), ScientificCorpusRefusal> {
        self.validate()?;
        if !available.contains(&self.manifest.identity.digest())
            || self
                .shards
                .iter()
                .any(|shard| !available.contains(&shard.identity.digest()))
        {
            return Err(ScientificCorpusRefusal::MissingResource);
        }
        Ok(())
    }

    pub fn validate_membership(
        &self,
        membership: &DatasetSplitMembership,
    ) -> Result<(), ScientificCorpusRefusal> {
        self.validate()?;
        membership.validate()?;
        if membership.dataset_identity != self.identity {
            return Err(ScientificCorpusRefusal::DatasetMismatch);
        }
        if !self.split_identities.contains(&membership.split_identity) {
            return Err(ScientificCorpusRefusal::UnknownSplit);
        }
        Ok(())
    }
}

impl DatasetSplitMembership {
    pub fn validate(&self) -> Result<(), ScientificCorpusRefusal> {
        nonzero(self.dataset_identity).map_err(ScientificCorpusRefusal::Observation)?;
        text(&self.split_identity).map_err(ScientificCorpusRefusal::Observation)?;
        if self.examples.is_empty() {
            return Err(ScientificCorpusRefusal::EmptyMembership);
        }
        if self.examples.len() > MAXIMUM_SPLIT_MEMBERS_PER_RECORD {
            return Err(ScientificCorpusRefusal::TooManyMembers);
        }
        if self.examples.contains(&[0; 32]) {
            return Err(ScientificCorpusRefusal::Observation(
                ScientificObservationRefusal::MissingIdentity,
            ));
        }
        if duplicate(self.examples.iter().copied()) {
            return Err(ScientificCorpusRefusal::DuplicateMember);
        }
        Ok(())
    }
}

pub fn prove_splits_disjoint(
    left: &DatasetSplitMembership,
    right: &DatasetSplitMembership,
) -> Result<(), ScientificCorpusRefusal> {
    left.validate()?;
    right.validate()?;
    if left.dataset_identity != right.dataset_identity {
        return Err(ScientificCorpusRefusal::DatasetMismatch);
    }
    if left.split_identity == right.split_identity {
        return Err(ScientificCorpusRefusal::DuplicateSplit);
    }
    if left
        .examples
        .iter()
        .any(|example| right.examples.contains(example))
    {
        return Err(ScientificCorpusRefusal::SplitLeakage);
    }
    Ok(())
}
