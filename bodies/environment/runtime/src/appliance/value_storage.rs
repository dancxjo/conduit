//! Canonical value backing for this finite appliance graph. Allocation is
//! preparation only; the existing kernel store never grows during Play.
use conduit_kernel::{HostedValueStore, StorageError};

// Every other checked wire Type is at most 683 bytes; best-fit allocation therefore
// has exactly the same eligible slot classes as the former 1024-byte small slots.
// Two Type-bounded canonical-output reservations coexist with seven smaller values during
// the admitted graph's routing. Eight slots exhausted transient item capacity
// in the full authored workload even though post-Step occupancy peaked at eight.
const CAPACITIES: [u32; 9] = [2274, 2274, 683, 683, 683, 683, 683, 683, 683];
const PAYLOAD_BYTES: u32 = 9_329;
// A changed checked reading Type must not silently outgrow this reviewed profile.
include!(concat!(env!("OUT_DIR"), "/sampling-reading-extent.rs"));
include!(concat!(env!("OUT_DIR"), "/sampling-branch-extents.rs"));

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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::ValueStorage;
    #[test]
    fn checked_capacity_classes_preserve_best_fit_for_every_live_slot_subset() {
        // Representative lengths at each checked Type/branch bound, plus
        // the full large class. No runtime value occupies the removed gap.
        for mask in 0u16..512 {
            for length in [
                0, 8, 21, 202, 224, 227, 242, 387, 571, 573, 656, 683, 1025, 1923, 2274,
            ] {
                let mut old = HostedValueStore::new_with_slot_capacities(
                    &[2274, 2274, 1024, 1024, 1024, 1024, 1024, 1024, 1024],
                    11_716,
                )
                .unwrap();
                let mut new = prepare_value_storage().unwrap();
                let mut references = alloc::vec::Vec::new();
                for bytes in [683; 7].into_iter().chain([2274; 2]) {
                    let payload = alloc::vec![0; bytes];
                    let a = old.store(&payload).unwrap();
                    let b = new.store(&payload).unwrap();
                    assert_eq!(a, b);
                    references.push((a, b));
                }
                for (index, (a, b)) in references.into_iter().enumerate() {
                    if mask & (1 << index) != 0 {
                        old.release(a).unwrap();
                        new.release(b).unwrap();
                    }
                }
                let payload = alloc::vec![0; length];
                // At a completely full store the tighter physical backing
                // can report ByteCapacityExceeded before ItemCapacityExceeded.
                // Successful admission and exact chosen slot must be equal.
                assert_eq!(
                    old.store(&payload).ok(),
                    new.store(&payload).ok(),
                    "mask={mask} length={length}"
                );
                assert_eq!(old.used_items(), new.used_items());
                assert_eq!(old.used_bytes(), new.used_bytes());
            }
        }
    }
}

/// Physical specialization only; local Source/Plan admission must subsequently
/// verify and bind the exact representation-aware producer/publication edge.
#[cfg(feature = "local-planning")]
pub fn prepare_compact_value_storage() -> Result<HostedValueStore, StorageError> {
    HostedValueStore::new_with_shared_compact_canonical_slots(
        &[CAPACITIES[2]; 9],
        crate::publication::source_reading::compact_layout(),
        PAYLOAD_BYTES,
    )
}
#[cfg(feature = "local-planning")]
pub fn compact_value_storage_bytes() -> Result<usize, StorageError> {
    HostedValueStore::shared_compact_canonical_storage_bytes(&[CAPACITIES[2]; 9])
}

#[cfg(feature = "local-planning")]
mod checked_extents {
    include!(concat!(env!("OUT_DIR"), "/sampling-value-extents.rs"));
}
/// The former large slots may now carry only the verified Result representation.
/// Every other routed Type must fit all nine unchanged small canonical buffers.
/// This checks Source facts, not lengths observed in a fixture or live sample.
#[cfg(feature = "local-planning")]
pub(super) fn permits_compact_route(kind: &str, reading: bool) -> bool {
    if reading {
        return kind == crate::publication::source_reading::compact_result_kind();
    }
    let maximum = match kind {
        conduit_time::TICK_VALUE_KIND => Some(conduit_time::TICK_ENCODED_LEN),
        conduit_core::EMPTY_INFO_ID => Some(0),
        kind => checked_extents::VALUE_EXTENTS
            .iter()
            .find(|(name, _)| *name == kind)
            .map(|(_, bytes)| *bytes),
    };
    maximum.is_some_and(|bytes| bytes <= CAPACITIES[2])
}
