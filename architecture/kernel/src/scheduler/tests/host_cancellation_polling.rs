//! Cancellation adapter polling and completion races over a large reserved table.
extern crate std;
use super::host_cancellation::scheduler_with_pending;
use super::*;

#[test]
fn cancellation_polling_skips_absent_settled_and_dispatched_obligations() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            for race in 0..3 {
                let mut scheduler = scheduler_with_pending::<_, _, 4096>(
                    FixedValueStore::<4, 8>::new(32).unwrap(),
                    FixedSignLog::<64>::new(64 * core::mem::size_of::<crate::KernelEvent>() as u32)
                        .unwrap(),
                );
                assert!(scheduler.next_host_cancellation().is_none());
                assert_eq!(scheduler.scan_counts.get().cancellation_pending_slots, 0);
                scheduler.step().unwrap();
                let request = scheduler.next_host_request().unwrap();
                scheduler.scan_counts.set(Default::default());
                assert!(scheduler.next_host_cancellation().is_none());
                assert_eq!(scheduler.scan_counts.get().cancellation_pending_slots, 0);
                scheduler.step().unwrap();
                scheduler.step().unwrap();
                let outcome = HostCallOutcome {
                    disposition: HostCallDisposition::Cancelled,
                    output: None,
                    failure: None,
                };
                if race == 1 {
                    scheduler
                        .complete_host_call(request.node, request.request, outcome)
                        .unwrap();
                    scheduler.scan_counts.set(Default::default());
                    assert!(scheduler.next_host_cancellation().is_none());
                    assert_eq!(scheduler.scan_counts.get().cancellation_pending_slots, 0);
                    assert_eq!(scheduler.pending_host_call_count(), 1);
                } else {
                    if race == 2 {
                        assert_eq!(
                            scheduler.complete_host_call(
                                request.node,
                                request.request,
                                HostCallOutcome {
                                    output: Some(request.input),
                                    ..outcome
                                }
                            ),
                            Err(SchedulerError::HostCallOutputExceeded)
                        );
                    }
                    let cancellation = scheduler.next_host_cancellation().unwrap();
                    assert_eq!(
                        (cancellation.node, cancellation.request),
                        (request.node, request.request)
                    );
                    scheduler.scan_counts.set(Default::default());
                    assert!(scheduler.next_host_cancellation().is_none());
                    assert_eq!(scheduler.scan_counts.get().cancellation_pending_slots, 0);
                    scheduler
                        .complete_host_call(request.node, request.request, outcome)
                        .unwrap();
                }
                scheduler.cancel().unwrap();
                scheduler.scan_counts.set(Default::default());
                assert!(scheduler.next_host_cancellation().is_none());
                assert_eq!(scheduler.scan_counts.get().cancellation_pending_slots, 0);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn rejecting_adapter_stops_after_last_undispatched_request_without_moving_cursor() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut scheduler = scheduler_with_pending::<_, _, 4096>(
                FixedValueStore::<4, 8>::new(32).unwrap(),
                FixedSignLog::<64>::new(64 * core::mem::size_of::<crate::KernelEvent>() as u32)
                    .unwrap(),
            );
            scheduler.step().unwrap();
            // Reserved tail slots cannot hold calls: wrap directly to the
            // first-free allocation domain, retaining the public cursor.
            for (cursor, probes) in [(0, 1), (4095, 1), (1, 1)] {
                scheduler.host_request_cursor = cursor;
                scheduler.scan_counts.set(Default::default());
                let mut calls = 0;
                let signs = scheduler.signs.entries;
                assert!(scheduler
                    .next_host_request_matching(|request| {
                        assert_eq!(request.node, NodeId(0));
                        calls += 1;
                        false
                    })
                    .is_none());
                assert_eq!(calls, 1);
                assert_eq!(scheduler.host_request_cursor, cursor);
                assert_eq!(scheduler.undispatched_host_count, 1);
                assert_eq!(scheduler.scan_counts.get().dispatch_pending_slots, probes);
                assert_eq!(scheduler.signs.entries, signs);
            }
            assert_eq!(scheduler.next_host_request().unwrap().node, NodeId(0));
            scheduler.scan_counts.set(Default::default());
            assert!(scheduler
                .next_host_request_matching(|_| panic!("already dispatched"))
                .is_none());
            assert_eq!(scheduler.scan_counts.get().dispatch_pending_slots, 0);
            scheduler.cancel().unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn sparse_reserved_host_adapter_polling_observation() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut scheduler = scheduler_with_pending::<_, _, 4096>(
                FixedValueStore::<4, 8>::new(32).unwrap(),
                FixedSignLog::<64>::new(64 * core::mem::size_of::<crate::KernelEvent>() as u32)
                    .unwrap(),
            );
            scheduler.step().unwrap();
            for batch in 0..8 {
                scheduler.host_request_cursor = 1;
                scheduler.scan_counts.set(Default::default());
                let signs = scheduler.signs.entries;
                let start = std::time::Instant::now();
                let mut callbacks = 0;
                for _ in 0..4096 {
                    assert!(std::hint::black_box(&mut scheduler)
                        .next_host_request_matching(|request| {
                            assert_eq!(request.node, NodeId(0));
                            callbacks += 1;
                            false
                        })
                        .is_none());
                }
                let wall_ns = start.elapsed().as_nanos();
                assert_eq!(callbacks, 4096);
                assert_eq!(scheduler.host_request_cursor, 1);
                assert_eq!(scheduler.undispatched_host_count, 1);
                assert_eq!(scheduler.signs.entries, signs);
                std::println!("HOST_ADAPTER_ROW {{\"batch\":{batch},\"polls\":4096,\"callbacks\":{callbacks},\"slot_probes\":{},\"wall_ns\":{wall_ns}}}", scheduler.scan_counts.get().dispatch_pending_slots);
            }
            assert_eq!(scheduler.next_host_request().unwrap().node, NodeId(0));
            scheduler.cancel().unwrap();
            assert_eq!(scheduler.values.used_items(), 0);
        })
        .unwrap()
        .join()
        .unwrap();
}
