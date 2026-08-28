use conduit_ai::{
    SimilarityMetric, VectorIndexMember, VectorIndexResourceRefusal, VectorIndexState,
};
use std::collections::BTreeSet;

use super::{
    hosted_query_work, is_zero_vector, HostedHnswProfile, HostedHnswRecord, HostedHnswRefusal,
    MAXIMUM_HOSTED_HNSW_DIMENSIONS, MAXIMUM_HOSTED_HNSW_ITEMS,
};

pub(super) fn validate_records<T>(
    state: &VectorIndexState,
    profile: HostedHnswProfile,
    records: &[HostedHnswRecord<T>],
) -> Result<(), HostedHnswRefusal> {
    if records.is_empty() {
        return Err(HostedHnswRefusal::EmptyIndex);
    }
    if records.len() > MAXIMUM_HOSTED_HNSW_ITEMS
        || records.len() > state.contract.bounds.maximum_items as usize
    {
        return Err(HostedHnswRefusal::ItemLimitExceeded);
    }
    let mut sources = BTreeSet::new();
    for entry in records {
        let record = &entry.record;
        record.validate().map_err(HostedHnswRefusal::Vector)?;
        if record.embedding.profile.dimensions > MAXIMUM_HOSTED_HNSW_DIMENSIONS {
            return Err(HostedHnswRefusal::DimensionLimitExceeded);
        }
        if profile.metric == SimilarityMetric::CosineSimilarity
            && is_zero_vector(&record.embedding.values)
        {
            return Err(HostedHnswRefusal::Vector(
                conduit_ai::VectorRefusal::ZeroVector,
            ));
        }
        if !sources.insert(record.source_identity.as_str()) {
            return Err(HostedHnswRefusal::DuplicateSource);
        }
        record
            .embedding
            .profile
            .compatibility(&records[0].record.embedding.profile, profile.metric)
            .map_err(HostedHnswRefusal::Vector)?;
        if entry.stored_bytes == 0 {
            return Err(HostedHnswRefusal::Resource(
                VectorIndexResourceRefusal::StorageLimitExceeded,
            ));
        }
    }
    let required = hosted_query_work(
        records.len(),
        records[0].record.embedding.profile.dimensions,
    )?;
    if required > state.contract.bounds.maximum_query_work_units {
        return Err(HostedHnswRefusal::QueryWorkLimitExceeded);
    }
    let stored = records.iter().try_fold(0_u64, |total, entry| {
        total
            .checked_add(entry.stored_bytes)
            .ok_or(HostedHnswRefusal::StorageAccountingOverflow)
    })?;
    if stored > state.contract.bounds.maximum_storage_bytes {
        return Err(HostedHnswRefusal::Resource(
            VectorIndexResourceRefusal::StorageLimitExceeded,
        ));
    }
    Ok(())
}

pub(super) fn record_members<T>(
    records: &[HostedHnswRecord<T>],
) -> Result<Vec<VectorIndexMember>, HostedHnswRefusal> {
    records
        .iter()
        .map(|entry| {
            Ok(VectorIndexMember {
                source_identity: entry.record.source_identity.clone(),
                stored_bytes: entry.stored_bytes,
            })
        })
        .collect()
}

pub(super) fn validate_membership<T>(
    state: &VectorIndexState,
    records: &[HostedHnswRecord<T>],
) -> Result<(), HostedHnswRefusal> {
    let state_sources: BTreeSet<_> = state
        .members()
        .iter()
        .map(|member| member.source_identity.as_str())
        .collect();
    let record_sources: BTreeSet<_> = records
        .iter()
        .map(|entry| entry.record.source_identity.as_str())
        .collect();
    if state_sources == record_sources && state_sources.len() == records.len() {
        Ok(())
    } else {
        Err(HostedHnswRefusal::IndexMembershipMismatch)
    }
}
