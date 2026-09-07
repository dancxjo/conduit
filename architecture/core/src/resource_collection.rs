//! Finite, storage-neutral Resource collections and immutable generations.

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceCollectionId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceEntryId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceGenerationId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceGeneration {
    pub entry_id: ResourceEntryId,
    pub generation_id: ResourceGenerationId,
    pub exact_name: String,
    pub observed_at: u64,
    pub relation: Option<ResourceEntryId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCollection {
    pub collection_id: ResourceCollectionId,
    maximum_entries: usize,
    entries: Vec<ResourceGeneration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceSelection<'a> {
    pub exact_name: Option<&'a str>,
    pub time_window: Option<(u64, u64)>,
    pub relation: Option<&'a ResourceEntryId>,
    pub latest_only: bool,
    pub maximum_candidates: usize,
    pub maximum_results: usize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ResourceCollectionRefusal {
    InvalidBounds,
    CollectionFull,
    DuplicateGeneration,
    MissingOrCorruptGeneration,
    CandidateBoundExceeded,
    ResultBoundExceeded,
    AmbiguousLatest,
}

impl ResourceCollection {
    pub fn new(
        id: ResourceCollectionId,
        maximum_entries: usize,
    ) -> Result<Self, ResourceCollectionRefusal> {
        if id.0.is_empty() || maximum_entries == 0 {
            return Err(ResourceCollectionRefusal::InvalidBounds);
        }
        Ok(Self {
            collection_id: id,
            maximum_entries,
            entries: Vec::new(),
        })
    }

    pub fn publish(
        &mut self,
        generation: ResourceGeneration,
    ) -> Result<(), ResourceCollectionRefusal> {
        if generation.entry_id.0.is_empty()
            || generation.generation_id.0.is_empty()
            || generation.exact_name.is_empty()
        {
            return Err(ResourceCollectionRefusal::MissingOrCorruptGeneration);
        }
        if self
            .entries
            .iter()
            .any(|existing| existing.generation_id == generation.generation_id)
        {
            return Err(ResourceCollectionRefusal::DuplicateGeneration);
        }
        if self.entries.len() == self.maximum_entries {
            return Err(ResourceCollectionRefusal::CollectionFull);
        }
        self.entries.push(generation);
        Ok(())
    }

    pub fn select<'a>(
        &'a self,
        query: &ResourceSelection<'_>,
    ) -> Result<Vec<&'a ResourceGeneration>, ResourceCollectionRefusal> {
        if query.maximum_candidates == 0 || query.maximum_results == 0 {
            return Err(ResourceCollectionRefusal::InvalidBounds);
        }
        let mut candidates = Vec::new();
        for entry in &self.entries {
            let matches = query.exact_name.is_none_or(|name| entry.exact_name == name)
                && query.time_window.is_none_or(|(from, through)| {
                    from <= through && entry.observed_at >= from && entry.observed_at <= through
                })
                && query
                    .relation
                    .is_none_or(|relation| entry.relation.as_ref() == Some(relation));
            if matches {
                if candidates.len() == query.maximum_candidates {
                    return Err(ResourceCollectionRefusal::CandidateBoundExceeded);
                }
                candidates.push(entry);
            }
        }
        candidates.sort_by(|a, b| {
            a.entry_id
                .cmp(&b.entry_id)
                .then(a.observed_at.cmp(&b.observed_at))
                .then(a.generation_id.cmp(&b.generation_id))
        });
        if query.latest_only && !candidates.is_empty() {
            let latest_time = candidates
                .iter()
                .map(|entry| entry.observed_at)
                .max()
                .unwrap_or(0);
            let latest: Vec<_> = candidates
                .into_iter()
                .filter(|entry| entry.observed_at == latest_time)
                .collect();
            if latest.len() != 1 {
                return Err(ResourceCollectionRefusal::AmbiguousLatest);
            }
            candidates = latest;
        }
        if candidates.len() > query.maximum_results {
            return Err(ResourceCollectionRefusal::ResultBoundExceeded);
        }
        Ok(candidates)
    }
}
