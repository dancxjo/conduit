#![cfg(feature = "alloc")]
use conduit_kernel::*;
#[path = "../../../semantics/ai/src/numeric_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn exact_full_arrays_and_one_under_preallocation_refusals() {
    for (local, remote) in [(1u16, 0u16), (1024, 0), (32, 4)] {
        let bytes = u32::from(local) * std::mem::size_of::<KernelEvent>() as u32;
        let remote_bytes = remote_sign_storage_bytes(remote).unwrap();
        let r = HostedSignLog::storage_reservation(local, bytes, remote, remote_bytes).unwrap();
        for (p, h) in [
            (
                r.preparation_requested_bytes_bound - 1,
                r.retained_heap_bytes_bound,
            ),
            (
                r.preparation_requested_bytes_bound,
                r.retained_heap_bytes_bound - 1,
            ),
        ] {
            let (result, o) = allocation_probe::observe(|| {
                HostedSignLog::new_with_storage_limits(local, bytes, remote, remote_bytes, p, h)
            });
            assert!(matches!(
                result,
                Err(HostedSignLogPreparationRefusal::Capacity)
            ));
            assert_eq!((o.allocations, o.reallocations), (0, 0));
        }
        let (result, o) = allocation_probe::observe(|| {
            HostedSignLog::new_with_storage_limits(
                local,
                bytes,
                remote,
                remote_bytes,
                r.preparation_requested_bytes_bound,
                r.retained_heap_bytes_bound,
            )
        });
        let (owner, receipt) = result.unwrap();
        assert_eq!(o.requested_bytes, receipt.preparation_requested_bytes_bound);
        assert_eq!(o.live_bytes, receipt.retained_heap_bytes_bound);
        assert_eq!(owner.owned_heap_bytes(), o.live_bytes);
        println!(
            "local={local} remote={remote} actual full array bytes={}",
            o.live_bytes
        );
    }
}
#[test]
fn unchanged_logical_sign_and_remote_behavior_no_runtime_allocation() {
    let local = 32;
    let remote = 4;
    let bytes = local as u32 * std::mem::size_of::<KernelEvent>() as u32;
    let rb = remote_sign_storage_bytes(remote).unwrap();
    let r = HostedSignLog::storage_reservation(local, bytes, remote, rb).unwrap();
    let (mut admitted, _) = HostedSignLog::new_with_storage_limits(
        local,
        bytes,
        remote,
        rb,
        r.preparation_requested_bytes_bound,
        r.retained_heap_bytes_bound,
    )
    .unwrap();
    let mut legacy = HostedSignLog::new_with_remote_storage(local, bytes, remote, rb).unwrap();
    let (_, o) = allocation_probe::observe(|| {
        for log in [&mut admitted, &mut legacy] {
            log.record(NodeId(0), None, None, KernelEventKind::BackCompleted)
                .unwrap();
            log.record_remote(
                NodeId(1),
                PortId(0),
                KernelEventKind::RemoteInputAdmitted,
                RemoteLifecycleIdentity {
                    endpoint: RemoteEndpointId(0),
                    cord: CordId(0),
                    direction: RemoteCordDirection::Ingress,
                    sequence: 9,
                },
            )
            .unwrap();
        }
    });
    assert_eq!((o.allocations, o.reallocations), (0, 0));
    assert_eq!(
        admitted.events().collect::<Vec<_>>(),
        legacy.events().collect::<Vec<_>>()
    );
    assert_eq!(admitted.used_bytes(), legacy.used_bytes());
    assert_eq!(admitted.remote_identity(1), legacy.remote_identity(1));
}

#[test]
fn legacy_budget_refusal_parity_precedes_allocation() {
    for local in 0..=2u16 {
        for remote in 0..=3u16 {
            for bytes in [
                0,
                1,
                u32::from(local) * std::mem::size_of::<KernelEvent>() as u32 + 1,
            ] {
                for remote_bytes in [0, 1, remote_sign_storage_bytes(remote).unwrap() + 1] {
                    let legacy =
                        HostedSignLog::new_with_remote_storage(local, bytes, remote, remote_bytes);
                    let (reservation, observed) = allocation_probe::observe(|| {
                        HostedSignLog::storage_reservation(local, bytes, remote, remote_bytes)
                    });
                    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
                    match (legacy, reservation) {
                        (Ok(_), Ok(_)) => (),
                        (Err(left), Err(HostedSignLogPreparationRefusal::Budget(right))) => {
                            assert_eq!(left, right)
                        }
                        _ => panic!("legacy reservation budget mismatch"),
                    }
                }
            }
        }
    }
}
