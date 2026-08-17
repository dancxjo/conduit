//! Bounded hosted HNSW realization below portable vector-search semantics.

use conduit_ai::{
    canonical_hit_order, EmbeddingProfile, SimilarityHit, SimilarityMetric, SimilarityQuery,
    VectorIndexHandle, VectorIndexMaintenanceKind, VectorIndexQueryAdmission,
    VectorIndexResourceRefusal, VectorIndexState, VectorRecord, VectorRefusal,
};
use conduit_core::ResourceBinding;
use instant_distance::{Builder, HnswMap, Point, Search};

mod validation;
use validation::{record_members, validate_membership, validate_records};

pub const HOSTED_HNSW_IMPLEMENTATION_ID: &str = "std/vector-index/hnsw@1";
pub const HOSTED_HNSW_LIBRARY_NAME: &str = "instant-distance";
pub const HOSTED_HNSW_LIBRARY_VERSION: &str = "0.6.1";
pub const HOSTED_HNSW_ALGORITHM: &str = "hnsw";
pub const MAXIMUM_HOSTED_HNSW_ITEMS: usize = 128;
pub const MAXIMUM_HOSTED_HNSW_DIMENSIONS: u32 = 32;
pub const MAXIMUM_HOSTED_HNSW_EF: u32 = 128;
pub const HOSTED_HNSW_WORK_FACTOR: u32 = 16;
pub const MAXIMUM_HOSTED_HNSW_IDENTITY_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedHnswProviderIdentity {
    pub implementation_identity: String,
    pub library_name: String,
    pub library_version: String,
    pub process_identity: String,
    pub algorithm: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostedHnswProfile {
    pub metric: SimilarityMetric,
    pub seed: u64,
    pub ef_construction: u32,
    pub ef_search: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedVectorSearchProofClass {
    ApproximateHnsw,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostedHnswSearchResult<T> {
    pub proof_class: HostedVectorSearchProofClass,
    pub provider: HostedHnswProviderIdentity,
    pub index_generation: u64,
    pub admitted_work_units: u32,
    pub approximate_candidate_count: u32,
    pub hits: Vec<SimilarityHit<T>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HostedHnswRecord<T> {
    pub record: VectorRecord<T>,
    /// Exact admitted storage accounting for this hosted record and graph share.
    pub stored_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedHnswMutationDisposition {
    ExplicitRebuildRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostedHnswRefusal {
    EmptyIdentity,
    IdentityTooLarge,
    InvalidProviderIdentity,
    InvalidProfile,
    EmptyIndex,
    ItemLimitExceeded,
    DimensionLimitExceeded,
    DuplicateSource,
    StorageAccountingOverflow,
    Vector(VectorRefusal),
    Resource(VectorIndexResourceRefusal),
    WrongMetric,
    StaleBackendGeneration,
    IndexMembershipMismatch,
    QueryWorkOverflow,
    QueryWorkLimitExceeded,
    PortableFilterUnsupported,
    PortableTemporalIntentUnsupported,
    ProviderLost,
    ExplicitRebuildRequired,
}

#[derive(Debug, Clone)]
struct HostedPoint {
    metric: SimilarityMetric,
    values: Vec<f32>,
}

impl Point for HostedPoint {
    fn distance(&self, other: &Self) -> f32 {
        match self.metric {
            SimilarityMetric::SquaredEuclideanDistance => self
                .values
                .iter()
                .zip(&other.values)
                .map(|(left, right)| {
                    let delta = left - right;
                    delta * delta
                })
                .sum(),
            SimilarityMetric::DotProductSimilarity => -self
                .values
                .iter()
                .zip(&other.values)
                .map(|(left, right)| left * right)
                .sum::<f32>(),
            SimilarityMetric::CosineSimilarity => {
                let mut dot = 0.0;
                let mut left_norm = 0.0;
                let mut right_norm = 0.0;
                for (left, right) in self.values.iter().zip(&other.values) {
                    dot += left * right;
                    left_norm += left * left;
                    right_norm += right * right;
                }
                1.0 - dot / (left_norm * right_norm).sqrt()
            }
        }
    }
}

pub struct HostedHnswVectorIndex<T> {
    provider: HostedHnswProviderIdentity,
    profile: HostedHnswProfile,
    embedding_profile: EmbeddingProfile,
    index_identity: String,
    generation: u64,
    records: Vec<HostedHnswRecord<T>>,
    map: HnswMap<HostedPoint, usize>,
    search: Search,
    provider_lost: bool,
}

impl HostedHnswProviderIdentity {
    pub fn reviewed(process_identity: impl Into<String>) -> Result<Self, HostedHnswRefusal> {
        let identity = Self {
            implementation_identity: HOSTED_HNSW_IMPLEMENTATION_ID.into(),
            library_name: HOSTED_HNSW_LIBRARY_NAME.into(),
            library_version: HOSTED_HNSW_LIBRARY_VERSION.into(),
            process_identity: process_identity.into(),
            algorithm: HOSTED_HNSW_ALGORITHM.into(),
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn validate(&self) -> Result<(), HostedHnswRefusal> {
        if self.process_identity.is_empty() {
            return Err(HostedHnswRefusal::EmptyIdentity);
        }
        if self.process_identity.len() > MAXIMUM_HOSTED_HNSW_IDENTITY_BYTES {
            return Err(HostedHnswRefusal::IdentityTooLarge);
        }
        if self.implementation_identity != HOSTED_HNSW_IMPLEMENTATION_ID
            || self.library_name != HOSTED_HNSW_LIBRARY_NAME
            || self.library_version != HOSTED_HNSW_LIBRARY_VERSION
            || self.algorithm != HOSTED_HNSW_ALGORITHM
        {
            return Err(HostedHnswRefusal::InvalidProviderIdentity);
        }
        Ok(())
    }
}

impl HostedHnswProfile {
    pub fn validate(self) -> Result<(), HostedHnswRefusal> {
        if self.ef_construction == 0
            || self.ef_construction > MAXIMUM_HOSTED_HNSW_EF
            || self.ef_search == 0
            || self.ef_search > MAXIMUM_HOSTED_HNSW_EF
        {
            return Err(HostedHnswRefusal::InvalidProfile);
        }
        Ok(())
    }
}

impl<T: Clone> HostedHnswVectorIndex<T> {
    #[allow(clippy::too_many_arguments)]
    pub fn rebuild(
        state: &mut VectorIndexState,
        maintenance_handle: &VectorIndexHandle,
        operation_identity: String,
        provider: HostedHnswProviderIdentity,
        profile: HostedHnswProfile,
        records: Vec<HostedHnswRecord<T>>,
    ) -> Result<Self, HostedHnswRefusal> {
        provider.validate()?;
        profile.validate()?;
        validate_records(state, profile, &records)?;
        let points = records
            .iter()
            .map(|entry| HostedPoint {
                metric: profile.metric,
                values: entry.record.embedding.values.clone(),
            })
            .collect();
        let values = (0..records.len()).collect();
        let members = record_members(&records)?;

        let authority_identity = maintenance_handle.authority_identity.clone();
        state
            .begin_maintenance(
                maintenance_handle,
                operation_identity.clone(),
                VectorIndexMaintenanceKind::Rebuild,
            )
            .map_err(HostedHnswRefusal::Resource)?;
        let active = state
            .handle(&authority_identity)
            .map_err(HostedHnswRefusal::Resource)?;
        let map = Builder::default()
            .seed(profile.seed)
            .ef_construction(profile.ef_construction as usize)
            .ef_search(profile.ef_search as usize)
            .build(points, values);
        state
            .complete_rebuild(
                &active,
                &operation_identity,
                records[0].record.embedding.profile.clone(),
                members,
            )
            .map_err(HostedHnswRefusal::Resource)?;

        Ok(Self {
            provider,
            profile,
            embedding_profile: state.contract.embedding_profile.clone(),
            index_identity: state.contract.index_identity.clone(),
            generation: state.contract.generation,
            records,
            map,
            search: Search::default(),
            provider_lost: false,
        })
    }

    pub fn provider(&self) -> &HostedHnswProviderIdentity {
        &self.provider
    }

    pub fn profile(&self) -> HostedHnswProfile {
        self.profile
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn mutation_disposition(&self) -> HostedHnswMutationDisposition {
        HostedHnswMutationDisposition::ExplicitRebuildRequired
    }

    pub fn refuse_hidden_mutation(&self) -> Result<(), HostedHnswRefusal> {
        Err(HostedHnswRefusal::ExplicitRebuildRequired)
    }

    pub fn mark_provider_lost(
        &mut self,
        state: &mut VectorIndexState,
        handle: &VectorIndexHandle,
    ) -> Result<u64, HostedHnswRefusal> {
        let generation = state
            .mark_unavailable(handle)
            .map_err(HostedHnswRefusal::Resource)?;
        self.provider_lost = true;
        Ok(generation)
    }

    pub fn query(
        &mut self,
        state: &VectorIndexState,
        handle: &VectorIndexHandle,
        query: &SimilarityQuery,
        admission: VectorIndexQueryAdmission,
        binding: &ResourceBinding,
    ) -> Result<HostedHnswSearchResult<T>, HostedHnswRefusal> {
        if self.provider_lost {
            return Err(HostedHnswRefusal::ProviderLost);
        }
        if state.contract.index_identity != self.index_identity
            || state.contract.generation != self.generation
        {
            return Err(HostedHnswRefusal::StaleBackendGeneration);
        }
        if state.contract.embedding_profile != self.embedding_profile {
            return Err(HostedHnswRefusal::Vector(
                VectorRefusal::ProfileIdentityMismatch,
            ));
        }
        if query.metric != self.profile.metric {
            return Err(HostedHnswRefusal::WrongMetric);
        }
        if !query.filters.is_empty() {
            return Err(HostedHnswRefusal::PortableFilterUnsupported);
        }
        if query.temporal_intent.is_some() {
            return Err(HostedHnswRefusal::PortableTemporalIntentUnsupported);
        }
        query.validate().map_err(HostedHnswRefusal::Vector)?;
        if self.profile.metric == SimilarityMetric::CosineSimilarity
            && is_zero_vector(&query.embedding.values)
        {
            return Err(HostedHnswRefusal::Vector(VectorRefusal::ZeroVector));
        }
        self.embedding_profile
            .compatibility(&query.embedding.profile, query.metric)
            .map_err(HostedHnswRefusal::Vector)?;
        state
            .admit_query(handle, admission, binding)
            .map_err(HostedHnswRefusal::Resource)?;
        if query.top_k > admission.maximum_results || query.top_k > self.profile.ef_search {
            return Err(HostedHnswRefusal::Resource(
                VectorIndexResourceRefusal::ResultLimitExceeded,
            ));
        }
        validate_membership(state, &self.records)?;
        let required_work =
            hosted_query_work(self.records.len(), self.embedding_profile.dimensions)?;
        if required_work > admission.work_units {
            return Err(HostedHnswRefusal::QueryWorkLimitExceeded);
        }

        let point = HostedPoint {
            metric: self.profile.metric,
            values: query.embedding.values.clone(),
        };
        let mut approximate = self
            .map
            .search(&point, &mut self.search)
            .map(|item| *item.value)
            .collect::<Vec<_>>();
        approximate.truncate(self.profile.ef_search as usize);
        let approximate_candidate_count =
            u32::try_from(approximate.len()).map_err(|_| HostedHnswRefusal::ItemLimitExceeded)?;
        let mut hits = Vec::with_capacity(core::cmp::min(approximate.len(), query.top_k as usize));
        for index in approximate {
            let record = &self.records[index].record;
            let score = query
                .score(&record.embedding)
                .map_err(HostedHnswRefusal::Vector)?;
            if !query
                .admits_score(score)
                .map_err(HostedHnswRefusal::Vector)?
            {
                continue;
            }
            hits.push(SimilarityHit {
                value: record.value.clone(),
                score,
                rank: 1,
                index_generation: self.generation,
                source_identity: record.source_identity.clone(),
                resource_identity: record.resource_identity.clone(),
                temporal_provenance: record.temporal_provenance.clone(),
            });
        }
        hits.sort_by(canonical_hit_order);
        hits.truncate(query.top_k as usize);
        for (index, hit) in hits.iter_mut().enumerate() {
            hit.rank = u32::try_from(index + 1).expect("hosted top-k is u32-bounded");
        }
        Ok(HostedHnswSearchResult {
            proof_class: HostedVectorSearchProofClass::ApproximateHnsw,
            provider: self.provider.clone(),
            index_generation: self.generation,
            admitted_work_units: admission.work_units,
            approximate_candidate_count,
            hits,
        })
    }
}

pub fn hosted_query_work(items: usize, dimensions: u32) -> Result<u32, HostedHnswRefusal> {
    u32::try_from(items)
        .map_err(|_| HostedHnswRefusal::QueryWorkOverflow)?
        .checked_mul(dimensions)
        .and_then(|work| work.checked_mul(HOSTED_HNSW_WORK_FACTOR))
        .ok_or(HostedHnswRefusal::QueryWorkOverflow)
}

pub(super) fn is_zero_vector(values: &[f32]) -> bool {
    values.iter().all(|value| *value == 0.0)
}
