//! Large corpus identity, finite manifests, and stable split membership.

use alloc::vec::Vec;

use crate::{
    duplicate, nonzero, text, DatasetDescriptor, DatasetExampleIdentity, DatasetExamplePage,
    DatasetSplitMembership, ScientificCorpusRefusal, ScientificObservationRefusal,
};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence};

pub const CORPUS_MANIFEST_PROFILE: &str = "data/corpus-manifest@1";
pub const MAXIMUM_CORPUS_SHARDS: usize = 64;
pub const MAXIMUM_DATASET_SPLITS: usize = 16;
pub const MAXIMUM_SPLIT_MEMBERS_PER_RECORD: usize = 4096;
const EXAMPLE_IDENTITY_BYTES: usize = 32;
const EXAMPLES_PER_PAGE: usize = DatasetExamplePage::MAXIMUM_BYTES / EXAMPLE_IDENTITY_BYTES;

impl From<ScientificObservationRefusal> for ScientificCorpusRefusal {
    fn from(refusal: ScientificObservationRefusal) -> Self {
        Self::observation(refusal).expect("an authored observation refusal is valid")
    }
}

impl DatasetDescriptor {
    pub fn validate(&self) -> Result<(), ScientificCorpusRefusal> {
        nonzero(self.identity).map_err(ScientificCorpusRefusal::from)?;
        text(&self.schema_profile).map_err(ScientificCorpusRefusal::from)?;
        if let Some(citation) = &self.citation_identity {
            text(citation).map_err(ScientificCorpusRefusal::from)?;
        }
        if let Some(license) = &self.license_profile {
            text(license).map_err(ScientificCorpusRefusal::from)?;
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
            text(split).map_err(ScientificCorpusRefusal::from)?;
        }
        if self
            .split_identities
            .iter()
            .enumerate()
            .any(|(index, split)| {
                self.split_identities
                    .iter()
                    .skip(index + 1)
                    .any(|candidate| candidate == split)
            })
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
    pub fn page<I>(identities: I) -> Result<DatasetExamplePage, ScientificCorpusRefusal>
    where
        I: IntoIterator<Item = DatasetExampleIdentity>,
    {
        let mut bytes = Vec::new();
        for identity in identities {
            bytes.extend_from_slice(identity.get());
        }
        if bytes.is_empty() || bytes.len() > DatasetExamplePage::MAXIMUM_BYTES {
            return Err(ScientificCorpusRefusal::InvalidMembershipPage);
        }
        DatasetExamplePage::new(
            BoundedBytes::new(&bytes).expect("a checked membership page fits its byte carrier"),
        )
        .map_err(|_| ScientificCorpusRefusal::InvalidMembershipPage)
    }

    pub fn pages<I>(
        identities: I,
    ) -> Result<BoundedSequence<DatasetExamplePage, 128>, ScientificCorpusRefusal>
    where
        I: IntoIterator<Item = DatasetExampleIdentity>,
    {
        let identities = identities.into_iter().collect::<Vec<_>>();
        if identities.is_empty() || identities.len() > MAXIMUM_SPLIT_MEMBERS_PER_RECORD {
            return Err(ScientificCorpusRefusal::TooManyMembers);
        }
        BoundedSequence::try_from_iter(
            identities
                .chunks(EXAMPLES_PER_PAGE)
                .map(|chunk| Self::page(chunk.iter().copied()))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(|_| ScientificCorpusRefusal::TooManyMembers)
    }

    pub fn identities(&self) -> impl Iterator<Item = DatasetExampleIdentity> + '_ {
        self.examples.iter().flat_map(|page| {
            page.get()
                .as_slice()
                .as_chunks::<EXAMPLE_IDENTITY_BYTES>()
                .0
                .iter()
                .map(|identity| {
                    DatasetExampleIdentity::new(*identity)
                        .expect("a dataset example identity is exactly 32 bytes")
                })
        })
    }

    pub fn validate(&self) -> Result<(), ScientificCorpusRefusal> {
        nonzero(self.dataset_identity).map_err(ScientificCorpusRefusal::from)?;
        text(&self.split_identity).map_err(ScientificCorpusRefusal::from)?;
        if self.examples.is_empty() {
            return Err(ScientificCorpusRefusal::EmptyMembership);
        }
        if self.examples.iter().enumerate().any(|(index, page)| {
            let length = page.get().as_slice().len();
            length == 0
                || length % EXAMPLE_IDENTITY_BYTES != 0
                || (index + 1 != self.examples.len()
                    && length != EXAMPLES_PER_PAGE * EXAMPLE_IDENTITY_BYTES)
        }) {
            return Err(ScientificCorpusRefusal::InvalidMembershipPage);
        }
        let example_count = self.identities().count();
        if example_count > MAXIMUM_SPLIT_MEMBERS_PER_RECORD {
            return Err(ScientificCorpusRefusal::TooManyMembers);
        }
        if self.identities().any(|identity| identity.get() == &[0; 32]) {
            return Err(ScientificObservationRefusal::MissingIdentity.into());
        }
        if duplicate(self.identities().map(|identity| *identity.get())) {
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
        .identities()
        .any(|example| right.identities().any(|candidate| candidate == example))
    {
        return Err(ScientificCorpusRefusal::SplitLeakage);
    }
    Ok(())
}
