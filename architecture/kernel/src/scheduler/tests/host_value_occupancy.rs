//! Cleanup checks only live ownership, including wrapped/coalescing queue rings.
extern crate std;
use super::*;
use crate::scheduler::HostValueDiscardRefusal;

type Owner = FixedScheduler<
    Driver,
    FixedValueStore<4, 8>,
    FixedSignLog<32>,
    1,
    1,
    PORTS,
    4096,
    0,
    0,
    1,
    4096,
>;
fn scheduler(policy: AssignedPressurePolicy) -> Owner {
    let mut routes = FixedRoutes::new(PORTS as u16);
    routes.seal().unwrap();
    FixedScheduler::new(
        [node([Some(CordId(0)), None])],
        [CordSpec::remote_ingress(
            CordId(0),
            RemoteEndpointId(0),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 2,
                byte_capacity: 16,
                pressure_policy: policy,
            },
        )],
        routes,
        [Driver::BlockedSink { cancelled: false }],
        FixedValueStore::new(32).unwrap(),
        FixedSignLog::new_with_remote_storage(
            32 * core::mem::size_of::<crate::KernelEvent>() as u32,
            32,
            crate::remote_sign_storage_bytes(32).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn host_value_cleanup_skips_empty_reserved_tables() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut scheduler = scheduler(AssignedPressurePolicy::default());
            let value = scheduler.store_host_value(b"temp").unwrap();
            scheduler.scan_counts.set(Default::default());
            scheduler.discard_host_value(value).unwrap();
            let work = scheduler.scan_counts.get();
            assert_eq!(work.value_owner_pending_slots, 0);
            assert_eq!(work.value_owner_queue_slots, 0);
            assert_eq!(scheduler.values().used_items(), 0);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn wrapped_and_coalescing_queued_values_remain_owned() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut scheduler = scheduler(AssignedPressurePolicy::CoalesceLatest);
            scheduler
                .admit_remote_input(RemoteEndpointId(0), CordId(0), 0, b"a")
                .unwrap();
            let old = scheduler.pop(0).unwrap();
            scheduler.discard_host_value(old).unwrap();
            for (sequence, bytes) in [(1, b"b"), (2, b"c"), (3, b"d")] {
                scheduler
                    .admit_remote_input(RemoteEndpointId(0), CordId(0), sequence, bytes)
                    .unwrap();
            }
            assert_eq!(scheduler.cords[0].head, 1);
            assert_eq!(scheduler.values().used_items(), 2);
            for slot in [0, 1] {
                let value = scheduler.queue_slots[slot].unwrap();
                scheduler.scan_counts.set(Default::default());
                assert_eq!(
                    scheduler.discard_host_value(value),
                    Err(HostValueDiscardRefusal::ValueOwned)
                );
                let work = scheduler.scan_counts.get();
                assert_eq!(work.value_owner_pending_slots, 0);
                assert!(work.value_owner_queue_slots <= 2);
                assert_eq!(scheduler.values().used_items(), 2);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
