//! Portable, bounded semantic relation of a body's current experience.

use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{
    ArtifactId, BaseImplementationId, BaseInstanceId, BoundedResourceRef, KindId, SignId,
    TemporalInstant,
};

use crate::{ExperienceTemporalPolicy, ExperienceTemporalRefusal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExperienceLimits {
    pub maximum_items: usize,
    pub maximum_current_items: usize,
    pub maximum_recent_items: usize,
    pub maximum_stale_items: usize,
    pub maximum_historical_items: usize,
    pub maximum_items_per_domain: usize,
    pub maximum_model_derived_items: usize,
    pub maximum_selected_memory_items: usize,
    pub maximum_source_refs: usize,
    pub maximum_relationships: usize,
    pub maximum_item_bytes: usize,
    pub maximum_encoded_bytes: usize,
    pub maximum_conflict_alternatives: usize,
    pub maximum_identity_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceDomain {
    Visual,
    Auditory,
    Location,
    BodyState,
    HumanUtterance,
    Recollection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceOrigin {
    Observation,
    HumanReported,
    Remembered,
    ModelDerived,
    Imagined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceTemporalRole {
    Current,
    Recent,
    Stale,
    Historical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceAvailability {
    Present,
    NotObserved,
    SourceUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceCertainty {
    Certain,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExperienceSourceRef {
    Sign(SignId),
    Resource(BoundedResourceRef),
    HumanStatement {
        statement_id: String,
    },
    MemoryRecord {
        record_id: String,
    },
    ImplementationRun {
        implementation_id: BaseImplementationId,
        provider_instance_id: BaseInstanceId,
        artifact_id: ArtifactId,
        run_id: String,
    },
    Source {
        source_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperienceItem {
    pub id: String,
    pub domain: ExperienceDomain,
    pub content_kind: KindId,
    pub encoded_content: Vec<u8>,
    pub origin: ExperienceOrigin,
    pub temporal_role: ExperienceTemporalRole,
    pub availability: ExperienceAvailability,
    pub certainty: ExperienceCertainty,
    pub observed_at: Option<TemporalInstant>,
    pub recorded_at: Option<TemporalInstant>,
    pub sources: Vec<ExperienceSourceRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceRelationKind {
    Relates,
    Contradicts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperienceRelation {
    pub subject_id: String,
    pub object_id: String,
    pub kind: ExperienceRelationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentExperience {
    limits: ExperienceLimits,
    reference_at: TemporalInstant,
    temporal_policy: ExperienceTemporalPolicy,
    items: Vec<ExperienceItem>,
    relationships: Vec<ExperienceRelation>,
    encoded_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceRefusal {
    InvalidLimits,
    EmptyIdentity,
    DuplicateIdentity,
    ItemCapacity,
    TemporalRoleCapacity,
    DomainCapacity,
    ModelDerivedCapacity,
    SelectedMemoryCapacity,
    ItemBytes,
    EncodedBytes,
    SourceCapacity,
    DuplicateSource,
    RelationshipCapacity,
    UnknownRelationshipEndpoint,
    DuplicateRelationship,
    ConflictAlternativeCapacity,
    InvalidEpistemicCombination,
    IdentityBytes,
    InvalidSource,
    InvalidTime,
    InvalidTemporalContext,
    TemporalClassification(ExperienceTemporalRefusal),
    TemporalRoleMismatch,
    ArithmeticOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperienceAdmissionError {
    pub refusal: ExperienceRefusal,
    pub item: Box<ExperienceItem>,
}

impl CurrentExperience {
    pub fn new(
        limits: ExperienceLimits,
        reference_at: TemporalInstant,
        temporal_policy: ExperienceTemporalPolicy,
    ) -> Result<Self, ExperienceRefusal> {
        if limits.maximum_items == 0
            || limits.maximum_current_items == 0
            || limits.maximum_recent_items == 0
            || limits.maximum_stale_items == 0
            || limits.maximum_historical_items == 0
            || limits.maximum_items_per_domain == 0
            || limits.maximum_model_derived_items == 0
            || limits.maximum_selected_memory_items == 0
            || limits.maximum_current_items > limits.maximum_items
            || limits.maximum_recent_items > limits.maximum_items
            || limits.maximum_stale_items > limits.maximum_items
            || limits.maximum_historical_items > limits.maximum_items
            || limits.maximum_items_per_domain > limits.maximum_items
            || limits.maximum_model_derived_items > limits.maximum_items
            || limits.maximum_selected_memory_items > limits.maximum_items
            || limits.maximum_source_refs == 0
            || limits.maximum_relationships == 0
            || limits.maximum_item_bytes == 0
            || limits.maximum_encoded_bytes == 0
            || limits.maximum_conflict_alternatives < 2
            || limits.maximum_identity_bytes == 0
        {
            return Err(ExperienceRefusal::InvalidLimits);
        }
        reference_at
            .validate()
            .map_err(|_| ExperienceRefusal::InvalidTemporalContext)?;
        temporal_policy
            .validate()
            .map_err(ExperienceRefusal::TemporalClassification)?;
        Ok(Self {
            items: Vec::with_capacity(limits.maximum_items),
            relationships: Vec::with_capacity(limits.maximum_relationships),
            limits,
            reference_at,
            temporal_policy,
            encoded_bytes: 0,
        })
    }

    pub fn items(&self) -> &[ExperienceItem] {
        &self.items
    }

    pub fn relationships(&self) -> &[ExperienceRelation] {
        &self.relationships
    }

    pub fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }

    /// Iterates all live observations, never recollection, inference, or imagination.
    pub fn current_observations<'a>(
        &'a self,
        content_kind: &'a KindId,
    ) -> impl Iterator<Item = &'a ExperienceItem> + 'a {
        self.items.iter().filter(move |item| {
            &item.content_kind == content_kind
                && item.origin == ExperienceOrigin::Observation
                && item.temporal_role == ExperienceTemporalRole::Current
                && item.availability == ExperienceAvailability::Present
        })
    }

    pub fn try_admit(&mut self, item: ExperienceItem) -> Result<(), ExperienceAdmissionError> {
        let refusal = self.validate_admission(&item).err();
        if let Some(refusal) = refusal {
            return Err(ExperienceAdmissionError {
                refusal,
                item: Box::new(item),
            });
        }
        self.encoded_bytes += item.encoded_content.len();
        self.items.push(item);
        Ok(())
    }

    pub fn relate(&mut self, relation: ExperienceRelation) -> Result<(), ExperienceRefusal> {
        if relation.subject_id.is_empty() || relation.object_id.is_empty() {
            return Err(ExperienceRefusal::EmptyIdentity);
        }
        if relation.subject_id.len() > self.limits.maximum_identity_bytes
            || relation.object_id.len() > self.limits.maximum_identity_bytes
        {
            return Err(ExperienceRefusal::IdentityBytes);
        }
        if !self.contains(&relation.subject_id) || !self.contains(&relation.object_id) {
            return Err(ExperienceRefusal::UnknownRelationshipEndpoint);
        }
        if self.relationships.contains(&relation) {
            return Err(ExperienceRefusal::DuplicateRelationship);
        }
        if self.relationships.len() == self.limits.maximum_relationships {
            return Err(ExperienceRefusal::RelationshipCapacity);
        }
        if relation.kind == ExperienceRelationKind::Contradicts {
            let alternatives = self
                .relationships
                .iter()
                .filter(|candidate| {
                    candidate.kind == ExperienceRelationKind::Contradicts
                        && (candidate.subject_id == relation.subject_id
                            || candidate.object_id == relation.subject_id
                            || candidate.subject_id == relation.object_id
                            || candidate.object_id == relation.object_id)
                })
                .count()
                + 2;
            if alternatives > self.limits.maximum_conflict_alternatives {
                return Err(ExperienceRefusal::ConflictAlternativeCapacity);
            }
        }
        self.relationships.push(relation);
        Ok(())
    }

    pub fn remove_source(&mut self, source: &ExperienceSourceRef) {
        for item in &mut self.items {
            let previous = item.sources.len();
            item.sources.retain(|candidate| candidate != source);
            if previous != item.sources.len() && item.sources.is_empty() {
                item.availability = ExperienceAvailability::SourceUnavailable;
                self.encoded_bytes -= item.encoded_content.len();
                item.encoded_content.clear();
            }
        }
    }

    fn contains(&self, id: &str) -> bool {
        self.items.iter().any(|item| item.id == id)
    }

    fn validate_admission(&self, item: &ExperienceItem) -> Result<(), ExperienceRefusal> {
        if item.id.is_empty() {
            return Err(ExperienceRefusal::EmptyIdentity);
        }
        if item.id.len() > self.limits.maximum_identity_bytes
            || item.content_kind.as_str().is_empty()
            || item.content_kind.as_str().len() > self.limits.maximum_identity_bytes
        {
            return Err(ExperienceRefusal::IdentityBytes);
        }
        if self.contains(&item.id) {
            return Err(ExperienceRefusal::DuplicateIdentity);
        }
        if self.items.len() == self.limits.maximum_items {
            return Err(ExperienceRefusal::ItemCapacity);
        }
        let temporal_capacity = match item.temporal_role {
            ExperienceTemporalRole::Current => self.limits.maximum_current_items,
            ExperienceTemporalRole::Recent => self.limits.maximum_recent_items,
            ExperienceTemporalRole::Stale => self.limits.maximum_stale_items,
            ExperienceTemporalRole::Historical => self.limits.maximum_historical_items,
        };
        if self
            .items
            .iter()
            .filter(|candidate| candidate.temporal_role == item.temporal_role)
            .count()
            == temporal_capacity
        {
            return Err(ExperienceRefusal::TemporalRoleCapacity);
        }
        if self
            .items
            .iter()
            .filter(|candidate| candidate.domain == item.domain)
            .count()
            == self.limits.maximum_items_per_domain
        {
            return Err(ExperienceRefusal::DomainCapacity);
        }
        if item.origin == ExperienceOrigin::ModelDerived
            && self
                .items
                .iter()
                .filter(|candidate| candidate.origin == ExperienceOrigin::ModelDerived)
                .count()
                == self.limits.maximum_model_derived_items
        {
            return Err(ExperienceRefusal::ModelDerivedCapacity);
        }
        if item.origin == ExperienceOrigin::Remembered
            && self
                .items
                .iter()
                .filter(|candidate| candidate.origin == ExperienceOrigin::Remembered)
                .count()
                == self.limits.maximum_selected_memory_items
        {
            return Err(ExperienceRefusal::SelectedMemoryCapacity);
        }
        if item.encoded_content.len() > self.limits.maximum_item_bytes {
            return Err(ExperienceRefusal::ItemBytes);
        }
        if item.sources.len() > self.limits.maximum_source_refs {
            return Err(ExperienceRefusal::SourceCapacity);
        }
        if item
            .sources
            .iter()
            .any(|source| !valid_source(source, self.limits.maximum_identity_bytes))
        {
            return Err(ExperienceRefusal::InvalidSource);
        }
        if item
            .sources
            .iter()
            .enumerate()
            .any(|(index, source)| item.sources[..index].contains(source))
        {
            return Err(ExperienceRefusal::DuplicateSource);
        }
        if item
            .observed_at
            .iter()
            .chain(item.recorded_at.iter())
            .any(|instant| instant.validate().is_err())
        {
            return Err(ExperienceRefusal::InvalidTime);
        }
        if item.origin == ExperienceOrigin::Remembered
            && item.temporal_role != ExperienceTemporalRole::Historical
            || item.availability != ExperienceAvailability::Present
                && !item.encoded_content.is_empty()
        {
            return Err(ExperienceRefusal::InvalidEpistemicCombination);
        }
        if item.availability == ExperienceAvailability::Present
            && matches!(
                item.origin,
                ExperienceOrigin::Observation
                    | ExperienceOrigin::HumanReported
                    | ExperienceOrigin::ModelDerived
            )
        {
            let observed_at = item
                .observed_at
                .as_ref()
                .ok_or(ExperienceRefusal::InvalidTime)?;
            let classified = self
                .temporal_policy
                .classify(observed_at, &self.reference_at)
                .map_err(ExperienceRefusal::TemporalClassification)?;
            if classified != item.temporal_role {
                return Err(ExperienceRefusal::TemporalRoleMismatch);
            }
        }
        let total = self
            .encoded_bytes
            .checked_add(item.encoded_content.len())
            .ok_or(ExperienceRefusal::ArithmeticOverflow)?;
        if total > self.limits.maximum_encoded_bytes {
            return Err(ExperienceRefusal::EncodedBytes);
        }
        Ok(())
    }
}

fn valid_source(source: &ExperienceSourceRef, maximum_identity_bytes: usize) -> bool {
    let identity = match source {
        ExperienceSourceRef::Sign(id) => Some(id.as_str()),
        ExperienceSourceRef::HumanStatement { statement_id } => Some(statement_id.as_str()),
        ExperienceSourceRef::MemoryRecord { record_id } => Some(record_id.as_str()),
        ExperienceSourceRef::Source { source_id } => Some(source_id.as_str()),
        ExperienceSourceRef::Resource(resource) => return resource.validate().is_ok(),
        ExperienceSourceRef::ImplementationRun {
            implementation_id,
            provider_instance_id,
            artifact_id,
            run_id,
        } => {
            return [
                implementation_id.as_str(),
                provider_instance_id.as_str(),
                artifact_id.as_str(),
                run_id,
            ]
            .iter()
            .all(|value| !value.is_empty() && value.len() <= maximum_identity_bytes)
        }
    };
    identity.is_some_and(|value| !value.is_empty() && value.len() <= maximum_identity_bytes)
}
