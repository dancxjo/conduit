#![cfg(feature = "alloc")]
use conduit_kernel::{HostedValueStore, ValueStorage};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn hosted_native_sidecar_is_prepared_once_and_mixed_storage_never_grows() {
    let mut values = HostedValueStore::new(4, 8, 32).unwrap();
    let (_, prepared) = allocation_probe::observe(|| values.prepare_native_u64());
    assert_eq!(prepared.allocations, 1);
    assert_eq!(prepared.reallocations, 0);
    let capacity = values.allocation_capacities();
    let (_, running) = allocation_probe::observe(|| {
        values.prepare_native_u64();
        for word in 0..256 {
            let a = values.store_native_u64(word).unwrap();
            let b = values.store(&word.to_le_bytes()).unwrap();
            assert_eq!(values.native_u64(a).unwrap(), Some(word));
            assert_eq!(values.get(b).unwrap(), word.to_le_bytes());
            values.retain(a).unwrap();
            values.release(a).unwrap();
            values.release(b).unwrap();
            values.release(a).unwrap();
        }
    });
    assert_eq!((running.allocations, running.reallocations), (0, 0));
    assert_eq!(values.allocation_capacities(), capacity);
    assert_eq!(values.used_items(), 0);
}
