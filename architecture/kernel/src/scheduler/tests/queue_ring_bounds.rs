//! Production queue operations over large, valid admitted ring layouts.
extern crate std;
use super::*;

type Ring =
    FixedScheduler<Driver, FixedValueStore<2, 1>, FixedSignLog<8>, 1, 1, PORTS, 66_000, 0, 0, 1, 1>;
fn scheduler(start: u16, capacity: u16) -> Ring {
    let mut routes = FixedRoutes::new(PORTS as u16);
    routes.seal().unwrap();
    FixedScheduler::new(
        [node([Some(CordId(0)), None])],
        [CordSpec::remote_ingress(
            CordId(0),
            RemoteEndpointId(0),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: start,
                item_capacity: capacity,
                byte_capacity: 2,
                pressure_policy: AssignedPressurePolicy::CoalesceLatest,
            },
        )],
        routes,
        [Driver::BlockedSink { cancelled: false }],
        FixedValueStore::new(2).unwrap(),
        FixedSignLog::new(8 * core::mem::size_of::<crate::KernelEvent>() as u32).unwrap(),
    )
    .unwrap()
}

#[test]
fn large_wrapped_queue_preserves_order_and_coalescing_without_u16_addition() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let mut scheduler = scheduler(0, 60_000);
            // An empty ring at this head is reachable after ordinary push/pop cycles.
            scheduler.cords[0].head = 59_000;
            let original = scheduler.values.store(b"").unwrap();
            for index in 0..60_000 {
                if index != 0 {
                    scheduler.values.retain(original).unwrap();
                }
                assert_eq!(scheduler.push(0, original), Ok(None));
            }
            let replacement = scheduler.values.store(b"x").unwrap();
            assert!(scheduler
                .payload_admissible(scheduler.cord_specs[0], &scheduler.cords[0], 1)
                .unwrap());
            assert_eq!(scheduler.push(0, replacement), Ok(Some(original)));
            scheduler.values.release(original).unwrap();
            assert_eq!(scheduler.queue_slots[58_999], Some(replacement));
            assert_eq!(scheduler.peek(0), Ok(Some(original)));
            for _ in 0..1_000 {
                assert_eq!(scheduler.pop(0), Ok(original));
                scheduler.values.release(original).unwrap();
            }
            assert_eq!(scheduler.cords[0].head, 0);
            assert_eq!(scheduler.cords[0].len, 59_000);
            assert_eq!(scheduler.queue_slots[58_999], Some(replacement));
            assert_eq!(
                scheduler.discard_host_value(replacement),
                Err(crate::scheduler::HostValueDiscardRefusal::ValueOwned)
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn admitted_queue_arena_can_cross_the_u16_slot_address_boundary() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let mut scheduler = scheduler(65_000, 600);
            scheduler.cords[0].head = 550;
            let value = scheduler.values.store(b"x").unwrap();
            assert_eq!(scheduler.push(0, value), Ok(None));
            assert_eq!(scheduler.queue_slots[65_550], Some(value));
            assert_eq!(scheduler.peek(0), Ok(Some(value)));
            assert_eq!(scheduler.pop(0), Ok(value));
            scheduler.discard_host_value(value).unwrap();
            assert_eq!(scheduler.values().used_items(), 0);
        })
        .unwrap()
        .join()
        .unwrap();
}
