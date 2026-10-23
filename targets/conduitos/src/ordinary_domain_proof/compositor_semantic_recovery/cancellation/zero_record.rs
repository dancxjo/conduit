//! Actual stopped-disposal witnesses; no samples, addresses or spare-capacity reads.
use super::*;
use crate::{
    arch::WholeZeroWitness,
    protected_compositor::display_owner::StagingZeroWitness,
    service_memory::{ServiceMemoryLayout, ServiceProfile},
};
use serde::Serialize;
const PREFIX: &[u8] = b"CONDUIT_COMPOSITOR_STOPPED_ZERO_WITNESS ";
const NAMES: [&str; 12] = [
    "pml4", "pdpt", "pd", "pt", "image", "exchange", "output", "stack", "metadata", "pixels",
    "scratch", "trap",
];
#[derive(Clone, Copy)]
pub(super) struct Audit {
    private: WholeZeroWitness,
    staging: StagingZeroWitness,
}
fn expected() -> [u64; 12] {
    let layout = ServiceMemoryLayout::compositor();
    [
        4096,
        4096,
        4096,
        layout.end().div_ceil(2 * 1024 * 1024) * 4096,
        layout.image().bytes(),
        layout.exchange().bytes(),
        layout.output().bytes(),
        layout.stack().bytes(),
        layout.metadata().bytes(),
        layout.pixels().unwrap().bytes(),
        layout.scratch().unwrap().bytes(),
        16384,
    ]
}
pub(super) fn validate(
    private: WholeZeroWitness,
    staging: StagingZeroWitness,
    token: crate::arch::ServiceZeroToken,
    before: DisplayObservation,
    disposed: DisplayObservation,
    capacity_bytes: usize,
) -> Audit {
    let full = expected();
    if private.token != token
        || private.profile != ServiceProfile::Compositor
        || private.instance != before.service.instance
        || disposed.service.instance != private.instance
        || !private.counts_valid
        || !private.all_zero
        || crate::arch::ZERO_REGIONS != 12
        || crate::arch::SERVICE_ZERO_OBSERVER_BYTES > 512
        || private.storage_bytes != full.iter().sum::<u64>()
        || private.checked_bytes != private.storage_bytes
        || private.storage_bytes != u64::from(disposed.current_cost.reserved_bytes)
        || private.regions.iter().zip(full).any(|(r, expected)| {
            r.expected_bytes != expected || r.checked_bytes != expected || !r.all_zero
        })
        || staging.instance != private.instance
        || staging.release_ordinal != token.ordinal()
        || token.ordinal() == 0
        || staging.initialized_words == 0
        || staging.checked_words != staging.initialized_words
        || staging.nonzero_words != 0
        || staging.capacity_words < staging.initialized_words
        || staging.initialized_words.checked_mul(4) != Some(before.staging_live_bytes)
        || before.staging_live_bytes != before.staging_prepared_bytes
        || staging.capacity_words.checked_mul(4) != Some(capacity_bytes)
        || disposed.staging_live_bytes != 0
    {
        refuse("cancel-whole-zero-binding");
    }
    Audit { private, staging }
}
#[derive(Serialize)]
struct Region {
    name: &'static str,
    expected_bytes: u64,
    checked_bytes: u64,
    all_zero: bool,
}
#[derive(Serialize)]
struct Private {
    instance: u64,
    release_ordinal: u64,
    storage_bytes: u64,
    checked_bytes: u64,
    counts_valid: bool,
    all_zero: bool,
    regions: [Region; 12],
}
#[derive(Serialize)]
struct Staging {
    instance: u64,
    release_ordinal: u64,
    initialized_words: usize,
    checked_words: usize,
    capacity_words: usize,
    nonzero_words: usize,
}
#[derive(Serialize)]
struct Record<'a> {
    schema: &'static str,
    identity: Identity<'a>,
    instance: u64,
    namespace: u64,
    release_ordinal: u64,
    profile: &'static str,
    instance_label_source: &'static str,
    private_observed_at: &'static str,
    staging_observed_at: &'static str,
    backend_teardown_ticks_include_scan: bool,
    service_disposal_ticks_include_scan: bool,
    owner_disposal_ticks_include_scan: bool,
    observer_storage_bytes: usize,
    private: Private,
    staging: Staging,
}
pub(super) fn emit(grant: &SharedDisplayGrant, guest: &[u8; 32], ack: AckRef, audit: Audit) {
    if ack.instance != audit.private.instance {
        refuse("cancel-zero-ack");
    }
    let private = audit.private;
    let record = Record {
        schema: "conduit.conduitos/compositor-stopped-zero-witness@1",
        identity: Identity::borrowed(grant.identity(), &crate::make::EMBEDDED_MAKE, guest),
        instance: private.instance,
        namespace: ack.namespace,
        release_ordinal: private.token.ordinal(),
        profile: "compositor",
        instance_label_source: "root-exclusive-disposal-window",
        private_observed_at: "before-lease-release",
        staging_observed_at: "after-volatile-wipe-before-free",
        backend_teardown_ticks_include_scan: false,
        service_disposal_ticks_include_scan: true,
        owner_disposal_ticks_include_scan: true,
        observer_storage_bytes: crate::arch::SERVICE_ZERO_OBSERVER_BYTES,
        private: Private {
            instance: private.instance,
            release_ordinal: private.token.ordinal(),
            storage_bytes: private.storage_bytes,
            checked_bytes: private.checked_bytes,
            counts_valid: private.counts_valid,
            all_zero: private.all_zero,
            regions: core::array::from_fn(|index| {
                let r = private.regions[index];
                Region {
                    name: NAMES[index],
                    expected_bytes: r.expected_bytes,
                    checked_bytes: r.checked_bytes,
                    all_zero: r.all_zero,
                }
            }),
        },
        staging: Staging {
            instance: audit.staging.instance,
            release_ordinal: audit.staging.release_ordinal,
            initialized_words: audit.staging.initialized_words,
            checked_words: audit.staging.checked_words,
            capacity_words: audit.staging.capacity_words,
            nonzero_words: audit.staging.nonzero_words,
        },
    };
    let mut buffer = [0u8; 4096];
    let length = serde_json_core::to_slice(&record, &mut buffer)
        .unwrap_or_else(|_| refuse("cancel-zero-record-bound"));
    if length + PREFIX.len() + 1 > buffer.len() {
        refuse("cancel-zero-line-bound");
    }
    arch::early_write(PREFIX);
    arch::early_write(&buffer[..length]);
    arch::early_write(b"\n");
}
