//! Canonical value backing for this finite appliance graph. Allocation is
//! preparation only; the existing kernel store never grows during Play.
use conduit_kernel::{HostedValueStore, StorageError};

// Two Type-bounded canonical-output reservations coexist with seven smaller values during
// the admitted graph's routing. Eight slots exhausted transient item capacity
// in the full authored workload even though post-Step occupancy peaked at eight.
const CAPACITIES: [u32; 9] = [2274, 2274, 1024, 1024, 1024, 1024, 1024, 1024, 1024];
const PAYLOAD_BYTES: u32 = 11_716;
// A changed checked reading Type must not silently outgrow this reviewed profile.
include!(concat!(env!("OUT_DIR"), "/sampling-reading-extent.rs"));

/// Backing bytes required before construction, including canonical slot
/// metadata on the compiling architecture. Allocator overhead, the store object,
/// drivers, assignment, signs and native providers are separate admission costs.
/// This is a storage profile, not a complete MCU memory-closure claim.
pub fn value_storage_bytes() -> Result<usize, StorageError> {
    HostedValueStore::canonical_storage_bytes(CAPACITIES)
}

/// Caller must admit `value_storage_bytes()` plus the other live costs before
/// entering this allocating constructor. No native provider or authority is
/// created. The profile is proved for the current authored appliance graph;
/// changing that graph or its output bounds requires renewing that proof.
pub fn prepare_value_storage() -> Result<HostedValueStore, StorageError> {
    HostedValueStore::new_with_slot_capacities(&CAPACITIES, PAYLOAD_BYTES)
}
