//! Closed whole-data witness for one actual stopped compositor cancellation.
use super::{ConduitosError, Identity};
use super::{Record, refusal};
use serde::Deserialize;
use serde_json::Value;
pub(super) const PREFIX: &str = "CONDUIT_COMPOSITOR_STOPPED_ZERO_WITNESS ";
// Exact v1 compositor layout: 2MiB image, 4KiB exchange, 64KiB output/stack,
// 256KiB metadata, 8MiB pixels/scratch, 16KiB trap and 3+12 page tables.
const COUNTS: [u64; 12] = [
    4096, 4096, 4096, 49152, 2097152, 4096, 65536, 65536, 262144, 8388608, 8388608, 16384,
];
const NAMES: [&str; 12] = [
    "pml4", "pdpt", "pd", "pt", "image", "exchange", "output", "stack", "metadata", "pixels",
    "scratch", "trap",
];
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Region {
    name: String,
    expected_bytes: u64,
    checked_bytes: u64,
    all_zero: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Private {
    instance: u64,
    release_ordinal: u64,
    storage_bytes: u64,
    checked_bytes: u64,
    counts_valid: bool,
    all_zero: bool,
    regions: [Region; 12],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Staging {
    instance: u64,
    release_ordinal: u64,
    initialized_words: u64,
    checked_words: u64,
    capacity_words: u64,
    nonzero_words: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Witness {
    schema: String,
    identity: Identity,
    instance: u64,
    namespace: u64,
    release_ordinal: u64,
    profile: String,
    instance_label_source: String,
    private_observed_at: String,
    staging_observed_at: String,
    backend_teardown_ticks_include_scan: bool,
    service_disposal_ticks_include_scan: bool,
    owner_disposal_ticks_include_scan: bool,
    observer_storage_bytes: u64,
    private: Private,
    staging: Staging,
}
pub(super) fn validate(payload: &str, records: &[Record]) -> Result<Value, ConduitosError> {
    if records.len() != 8 {
        return Err(refusal());
    }
    let witness: Witness = serde_json::from_str(payload).map_err(|_| refusal())?;
    let first = &records[0];
    let before = &records[3];
    let disposed = &records[5];
    let ack = records[2].ack.0.as_ref().ok_or_else(refusal)?;
    let full = COUNTS.iter().sum::<u64>();
    let initialized = witness
        .staging
        .initialized_words
        .checked_mul(4)
        .ok_or_else(refusal)?;
    let capacity = witness
        .staging
        .capacity_words
        .checked_mul(4)
        .ok_or_else(refusal)?;
    if witness.schema != "conduit.conduitos/compositor-stopped-zero-witness@1"
        || witness.identity != first.identity
        || witness.instance != before.instance
        || witness.instance != disposed.instance
        || witness.instance != ack.instance
        || witness.namespace != ack.namespace
        || witness.namespace == 0
        || witness.release_ordinal == 0
        || witness.profile != "compositor"
        || witness.instance_label_source != "root-exclusive-disposal-window"
        || witness.private_observed_at != "before-lease-release"
        || witness.staging_observed_at != "after-volatile-wipe-before-free"
        || witness.backend_teardown_ticks_include_scan
        || !witness.service_disposal_ticks_include_scan
        || !witness.owner_disposal_ticks_include_scan
        || witness.observer_storage_bytes == 0
        || witness.observer_storage_bytes > 512
        || witness.private.instance != witness.instance
        || witness.staging.instance != witness.instance
        || witness.private.release_ordinal != witness.release_ordinal
        || witness.staging.release_ordinal != witness.release_ordinal
        || witness.private.storage_bytes != full
        || witness.private.checked_bytes != full
        || !witness.private.counts_valid
        || !witness.private.all_zero
        || witness.private.storage_bytes != u64::from(disposed.cost.reserved_bytes)
        || witness
            .private
            .regions
            .iter()
            .zip(NAMES)
            .zip(COUNTS)
            .any(|((r, name), count)| {
                r.name != name
                    || r.expected_bytes != count
                    || r.checked_bytes != count
                    || !r.all_zero
            })
        || witness.staging.initialized_words != 2_097_152
        || witness.staging.checked_words != witness.staging.initialized_words
        || initialized != before.staging_live_bytes as u64
        || initialized != before.staging_prepared_bytes as u64
        || capacity < initialized
        || witness.staging.nonzero_words != 0
        || disposed.staging_live_bytes != 0
    {
        return Err(refusal());
    }
    serde_json::from_str(payload).map_err(|_| refusal())
}
#[cfg(test)]
#[path = "zero_receipt/tests.rs"]
mod tests;
