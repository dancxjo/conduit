use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::{
    FiniteEmbedding, StructuredResultInvalidity, TemporalProvenance, TemporalRetrievalIntent,
    MAXIMUM_EMBEDDING_DIMENSIONS,
};

pub const MAXIMUM_VECTOR_IDENTITY_BYTES: usize = 256;
pub const MAXIMUM_VECTOR_METADATA: usize = 32;
pub const MAXIMUM_VECTOR_FILTERS: usize = 32;
pub const MAXIMUM_VECTOR_METADATA_KEY_BYTES: usize = 64;
pub const MAXIMUM_VECTOR_METADATA_VALUE_BYTES: usize = 1_024;
pub const MAXIMUM_SIMILARITY_TOP_K: u32 = 1_024;
const UNIT_NORM_TOLERANCE: f32 = 0.000_01;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimilarityMetric {
    CosineSimilarity,
    DotProductSimilarity,
    SquaredEuclideanDistance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingNormalization {
    None,
    UnitLength,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibleMetrics {
    pub cosine_similarity: bool,
    pub dot_product_similarity: bool,
    pub squared_euclidean_distance: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingProfile {
    pub identity: String,
    pub semantic_space_identity: String,
    pub model_identity: String,
    pub provider_identity: String,
    pub dimensions: u32,
    pub normalization: EmbeddingNormalization,
    pub compatible_metrics: CompatibleMetrics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embedding {
    pub profile: EmbeddingProfile,
    pub values: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorMetadata {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorRecord<T> {
    pub value: T,
    pub embedding: Embedding,
    pub source_identity: String,
    pub resource_identity: String,
    pub metadata: Vec<VectorMetadata>,
    pub temporal_provenance: Option<TemporalProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataFilter {
    Equal { key: String, value: String },
    Present { key: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SimilarityThreshold {
    MinimumSimilarity(f32),
    MaximumSquaredDistance(f32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimilarityQuery {
    pub embedding: Embedding,
    pub metric: SimilarityMetric,
    pub top_k: u32,
    pub threshold: Option<SimilarityThreshold>,
    pub filters: Vec<MetadataFilter>,
    pub temporal_intent: Option<TemporalRetrievalIntent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SimilarityScore {
    Similarity(f32),
    SquaredDistance(f32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimilarityHit<T> {
    pub value: T,
    pub score: SimilarityScore,
    pub rank: u32,
    pub index_generation: u64,
    pub source_identity: String,
    pub resource_identity: String,
    pub temporal_provenance: Option<TemporalProvenance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorRefusal {
    EmptyIdentity,
    IdentityTooLarge,
    ZeroDimensions,
    DimensionLimitExceeded,
    NoCompatibleMetric,
    InvalidEmbedding,
    NormalizationMismatch,
    ProfileIdentityMismatch,
    SemanticSpaceMismatch,
    ModelMismatch,
    ProviderMismatch,
    DimensionMismatch,
    MetricNotCompatible,
    ZeroVector,
    NonFiniteScore,
    TopKZero,
    TopKTooLarge,
    TooManyMetadata,
    TooManyFilters,
    InvalidMetadata,
    DuplicateMetadata,
    ThresholdMetricMismatch,
    InvalidThreshold,
    ThresholdNotMet,
    InvalidTemporalIntent,
    InvalidTemporalProvenance,
    RankZero,
    RankExceedsTopK,
}

impl CompatibleMetrics {
    pub fn admits(self, metric: SimilarityMetric) -> bool {
        match metric {
            SimilarityMetric::CosineSimilarity => self.cosine_similarity,
            SimilarityMetric::DotProductSimilarity => self.dot_product_similarity,
            SimilarityMetric::SquaredEuclideanDistance => self.squared_euclidean_distance,
        }
    }

    fn any(self) -> bool {
        self.cosine_similarity || self.dot_product_similarity || self.squared_euclidean_distance
    }
}

impl EmbeddingProfile {
    pub fn validate(&self) -> Result<(), VectorRefusal> {
        for identity in [
            &self.identity,
            &self.semantic_space_identity,
            &self.model_identity,
            &self.provider_identity,
        ] {
            if identity.is_empty() {
                return Err(VectorRefusal::EmptyIdentity);
            }
            if identity.len() > MAXIMUM_VECTOR_IDENTITY_BYTES {
                return Err(VectorRefusal::IdentityTooLarge);
            }
        }
        if self.dimensions == 0 {
            return Err(VectorRefusal::ZeroDimensions);
        }
        if self.dimensions as usize > MAXIMUM_EMBEDDING_DIMENSIONS {
            return Err(VectorRefusal::DimensionLimitExceeded);
        }
        if !self.compatible_metrics.any() {
            return Err(VectorRefusal::NoCompatibleMetric);
        }
        Ok(())
    }

    pub fn compatibility(
        &self,
        other: &Self,
        metric: SimilarityMetric,
    ) -> Result<(), VectorRefusal> {
        self.validate()?;
        other.validate()?;
        if self.identity != other.identity {
            return Err(VectorRefusal::ProfileIdentityMismatch);
        }
        if self.semantic_space_identity != other.semantic_space_identity {
            return Err(VectorRefusal::SemanticSpaceMismatch);
        }
        if self.model_identity != other.model_identity {
            return Err(VectorRefusal::ModelMismatch);
        }
        if self.provider_identity != other.provider_identity {
            return Err(VectorRefusal::ProviderMismatch);
        }
        if self.dimensions != other.dimensions {
            return Err(VectorRefusal::DimensionMismatch);
        }
        if self.normalization != other.normalization {
            return Err(VectorRefusal::NormalizationMismatch);
        }
        if !self.compatible_metrics.admits(metric) || !other.compatible_metrics.admits(metric) {
            return Err(VectorRefusal::MetricNotCompatible);
        }
        Ok(())
    }
}

impl Embedding {
    pub fn from_finite(
        profile: EmbeddingProfile,
        embedding: FiniteEmbedding,
    ) -> Result<Self, VectorRefusal> {
        if embedding.profile_identity != profile.identity {
            return Err(VectorRefusal::ProfileIdentityMismatch);
        }
        let result = Self {
            profile,
            values: embedding.values,
        };
        if embedding.dimensions != result.profile.dimensions {
            return Err(VectorRefusal::DimensionMismatch);
        }
        result.validate()?;
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), VectorRefusal> {
        self.profile.validate()?;
        FiniteEmbedding {
            profile_identity: self.profile.identity.clone(),
            dimensions: self.profile.dimensions,
            values: self.values.clone(),
        }
        .validate()
        .map_err(|_: StructuredResultInvalidity| VectorRefusal::InvalidEmbedding)?;
        if self.profile.normalization == EmbeddingNormalization::UnitLength {
            let norm_squared = dot(&self.values, &self.values)?;
            if (norm_squared - 1.0).abs() > UNIT_NORM_TOLERANCE {
                return Err(VectorRefusal::NormalizationMismatch);
            }
        }
        Ok(())
    }
}

impl<T> VectorRecord<T> {
    pub fn validate(&self) -> Result<(), VectorRefusal> {
        self.embedding.validate()?;
        validate_identity(&self.source_identity)?;
        validate_identity(&self.resource_identity)?;
        validate_metadata(&self.metadata)?;
        validate_temporal_provenance(self.temporal_provenance.as_ref())
    }
}

impl SimilarityQuery {
    pub fn validate(&self) -> Result<(), VectorRefusal> {
        self.embedding.validate()?;
        if !self
            .embedding
            .profile
            .compatible_metrics
            .admits(self.metric)
        {
            return Err(VectorRefusal::MetricNotCompatible);
        }
        if self.top_k == 0 {
            return Err(VectorRefusal::TopKZero);
        }
        if self.top_k > MAXIMUM_SIMILARITY_TOP_K {
            return Err(VectorRefusal::TopKTooLarge);
        }
        if self.filters.len() > MAXIMUM_VECTOR_FILTERS {
            return Err(VectorRefusal::TooManyFilters);
        }
        for filter in &self.filters {
            match filter {
                MetadataFilter::Equal { key, value } => validate_metadata_member(key, value)?,
                MetadataFilter::Present { key } => validate_metadata_member(key, "present")?,
            }
        }
        if let Some(threshold) = self.threshold {
            validate_threshold(self.metric, threshold)?;
        }
        if self
            .temporal_intent
            .as_ref()
            .is_some_and(|intent| intent.validate().is_err())
        {
            return Err(VectorRefusal::InvalidTemporalIntent);
        }
        Ok(())
    }

    pub fn score(&self, candidate: &Embedding) -> Result<SimilarityScore, VectorRefusal> {
        self.validate()?;
        candidate.validate()?;
        self.embedding
            .profile
            .compatibility(&candidate.profile, self.metric)?;
        let value = match self.metric {
            SimilarityMetric::DotProductSimilarity => {
                SimilarityScore::Similarity(dot(&self.embedding.values, &candidate.values)?)
            }
            SimilarityMetric::CosineSimilarity => {
                let numerator = dot(&self.embedding.values, &candidate.values)?;
                let left = dot(&self.embedding.values, &self.embedding.values)?;
                let right = dot(&candidate.values, &candidate.values)?;
                if left == 0.0 || right == 0.0 {
                    return Err(VectorRefusal::ZeroVector);
                }
                SimilarityScore::Similarity(numerator / libm::sqrtf(left * right))
            }
            SimilarityMetric::SquaredEuclideanDistance => {
                let mut total = 0.0;
                for (left, right) in self.embedding.values.iter().zip(&candidate.values) {
                    let delta = left - right;
                    total += delta * delta;
                }
                SimilarityScore::SquaredDistance(total)
            }
        };
        if !score_value(value).is_finite() {
            return Err(VectorRefusal::NonFiniteScore);
        }
        Ok(value)
    }

    pub fn admits_score(&self, score: SimilarityScore) -> Result<bool, VectorRefusal> {
        self.validate()?;
        validate_score_metric(self.metric, score)?;
        match (self.threshold, score) {
            (None, _) => Ok(true),
            (
                Some(SimilarityThreshold::MinimumSimilarity(minimum)),
                SimilarityScore::Similarity(value),
            ) => Ok(value >= minimum),
            (
                Some(SimilarityThreshold::MaximumSquaredDistance(maximum)),
                SimilarityScore::SquaredDistance(value),
            ) => Ok(value <= maximum),
            _ => Err(VectorRefusal::ThresholdMetricMismatch),
        }
    }
}

impl<T> SimilarityHit<T> {
    pub fn validate_for(&self, query: &SimilarityQuery) -> Result<(), VectorRefusal> {
        query.validate()?;
        validate_identity(&self.source_identity)?;
        validate_identity(&self.resource_identity)?;
        validate_temporal_provenance(self.temporal_provenance.as_ref())?;
        if self.rank == 0 {
            return Err(VectorRefusal::RankZero);
        }
        if self.rank > query.top_k {
            return Err(VectorRefusal::RankExceedsTopK);
        }
        if !score_value(self.score).is_finite() {
            return Err(VectorRefusal::NonFiniteScore);
        }
        if !query.admits_score(self.score)? {
            return Err(VectorRefusal::ThresholdNotMet);
        }
        Ok(())
    }
}

pub fn canonical_hit_order<T>(
    left: &SimilarityHit<T>,
    right: &SimilarityHit<T>,
) -> core::cmp::Ordering {
    let score_order = match (left.score, right.score) {
        (SimilarityScore::Similarity(left), SimilarityScore::Similarity(right)) => {
            right.total_cmp(&left)
        }
        (SimilarityScore::SquaredDistance(left), SimilarityScore::SquaredDistance(right)) => {
            left.total_cmp(&right)
        }
        (SimilarityScore::Similarity(_), SimilarityScore::SquaredDistance(_)) => {
            core::cmp::Ordering::Less
        }
        (SimilarityScore::SquaredDistance(_), SimilarityScore::Similarity(_)) => {
            core::cmp::Ordering::Greater
        }
    };
    score_order
        .then_with(|| left.source_identity.cmp(&right.source_identity))
        .then_with(|| left.resource_identity.cmp(&right.resource_identity))
}

fn dot(left: &[f32], right: &[f32]) -> Result<f32, VectorRefusal> {
    if left.len() != right.len() {
        return Err(VectorRefusal::DimensionMismatch);
    }
    let mut total = 0.0;
    for (left, right) in left.iter().zip(right) {
        total += left * right;
    }
    if !total.is_finite() {
        return Err(VectorRefusal::NonFiniteScore);
    }
    Ok(total)
}

fn score_value(score: SimilarityScore) -> f32 {
    match score {
        SimilarityScore::Similarity(value) | SimilarityScore::SquaredDistance(value) => value,
    }
}

fn validate_score_metric(
    metric: SimilarityMetric,
    score: SimilarityScore,
) -> Result<(), VectorRefusal> {
    match (metric, score) {
        (
            SimilarityMetric::CosineSimilarity | SimilarityMetric::DotProductSimilarity,
            SimilarityScore::Similarity(_),
        )
        | (SimilarityMetric::SquaredEuclideanDistance, SimilarityScore::SquaredDistance(_)) => {
            Ok(())
        }
        _ => Err(VectorRefusal::ThresholdMetricMismatch),
    }
}

fn validate_temporal_provenance(
    provenance: Option<&TemporalProvenance>,
) -> Result<(), VectorRefusal> {
    if provenance.is_some_and(|provenance| provenance.validate().is_err()) {
        Err(VectorRefusal::InvalidTemporalProvenance)
    } else {
        Ok(())
    }
}

fn validate_identity(identity: &str) -> Result<(), VectorRefusal> {
    if identity.is_empty() {
        return Err(VectorRefusal::EmptyIdentity);
    }
    if identity.len() > MAXIMUM_VECTOR_IDENTITY_BYTES {
        return Err(VectorRefusal::IdentityTooLarge);
    }
    Ok(())
}

fn validate_metadata(metadata: &[VectorMetadata]) -> Result<(), VectorRefusal> {
    if metadata.len() > MAXIMUM_VECTOR_METADATA {
        return Err(VectorRefusal::TooManyMetadata);
    }
    for member in metadata {
        validate_metadata_member(&member.key, &member.value)?;
    }
    if metadata.iter().enumerate().any(|(index, member)| {
        metadata[index + 1..]
            .iter()
            .any(|candidate| candidate.key == member.key)
    }) {
        return Err(VectorRefusal::DuplicateMetadata);
    }
    Ok(())
}

fn validate_metadata_member(key: &str, value: &str) -> Result<(), VectorRefusal> {
    if key.is_empty()
        || value.is_empty()
        || key.len() > MAXIMUM_VECTOR_METADATA_KEY_BYTES
        || value.len() > MAXIMUM_VECTOR_METADATA_VALUE_BYTES
    {
        return Err(VectorRefusal::InvalidMetadata);
    }
    Ok(())
}

fn validate_threshold(
    metric: SimilarityMetric,
    threshold: SimilarityThreshold,
) -> Result<(), VectorRefusal> {
    let valid = match (metric, threshold) {
        (
            SimilarityMetric::CosineSimilarity | SimilarityMetric::DotProductSimilarity,
            SimilarityThreshold::MinimumSimilarity(value),
        ) => value.is_finite(),
        (
            SimilarityMetric::SquaredEuclideanDistance,
            SimilarityThreshold::MaximumSquaredDistance(value),
        ) => value.is_finite() && value >= 0.0,
        _ => return Err(VectorRefusal::ThresholdMetricMismatch),
    };
    if valid {
        Ok(())
    } else {
        Err(VectorRefusal::InvalidThreshold)
    }
}
