//! Earlier-slot reinsertion while a higher cancellation remains queued.
use super::*;
type Pair = FixedScheduler<ManyBack, ObservedValues<4>, FixedSignLog<128>, 2, 2, 1, 2, 2, 0, 2, 4>;
fn pair() -> (Pair, ValueRef) {
    let mut values = ObservedValues::<4>::new();
    let input = values.store(&[]).unwrap();
    let mut routes = FixedRoutes::new(1);
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::new(1);
    for node in 0..2 {
        bindings
            .install(
                NodeId(node),
                HostCallBinding {
                    call: HostCallId(0),
                    maximum_input_bytes: 0,
                    maximum_output_bytes: 1,
                },
            )
            .unwrap();
    }
    bindings.seal().unwrap();
    let scheduler = Pair::new_with_host_calls(
        core::array::from_fn(|node| NodeSpec {
            input_cords: [Some(CordId(node as u16))],
            maximum_step_fuel: 8,
        }),
        core::array::from_fn(|node| {
            CordSpec::remote_ingress(
                CordId(node as u16),
                RemoteEndpointId(node as u16),
                (NodeId(node as u16), PortId(0)),
                CordCapacity {
                    slot_start: node as u16,
                    item_capacity: 1,
                    byte_capacity: 1,
                    pressure_policy: Default::default(),
                },
            )
        }),
        routes,
        bindings,
        [ManyBack {
            input,
            phase: 0,
            output: false,
            cancelled: false,
        }; 2],
        values,
        FixedSignLog::new_with_remote_storage(
            128 * core::mem::size_of::<crate::KernelEvent>() as u32,
            8,
            crate::remote_sign_storage_bytes(8).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    (scheduler, input)
}
fn step(scheduler: &mut Pair, node: u16) {
    assert_eq!(
        scheduler.step(),
        Ok(SchedulerStatus::Progress { node: NodeId(node) })
    );
}
fn request(scheduler: &mut Pair, node: u16, id: u32) {
    let request = scheduler.next_host_request().unwrap();
    assert_eq!(
        (request.node, request.request),
        (NodeId(node), RequestId(id))
    );
}
fn control(scheduler: &mut Pair, node: u16, sequence: u64) {
    assert_eq!(
        scheduler.admit_remote_input(RemoteEndpointId(node), CordId(node), sequence, &[1]),
        Ok(RemoteIngressOutcome::Accepted { sequence })
    );
}
fn complete(scheduler: &mut Pair, node: u16, id: u32) {
    scheduler
        .complete_host_call(
            NodeId(node),
            RequestId(id),
            HostCallOutcome {
                disposition: HostCallDisposition::Cancelled,
                output: None,
                failure: None,
            },
        )
        .unwrap();
}
fn poll(scheduler: &mut Pair, node: u16, id: u32, observed: &mut [u32; 4], count: &mut usize) {
    let cancellation = scheduler.next_host_cancellation().unwrap();
    assert_eq!(
        (cancellation.node, cancellation.request),
        (NodeId(node), RequestId(id))
    );
    observed[*count] = u32::from(cancellation.node.0) * 10 + cancellation.request.0;
    *count += 1;
}
fn interleave(settled: Option<u16>, cancel_all: bool) {
    let (mut scheduler, input) = pair();
    let mut observed = [u32::MAX; 4];
    let mut count = 0;
    for node in 0..2 {
        step(&mut scheduler, node);
        request(&mut scheduler, node, 0);
    }
    for node in 0..2 {
        control(&mut scheduler, node, 0);
    }
    for node in 0..2 {
        step(&mut scheduler, node);
    }
    poll(&mut scheduler, 0, 0, &mut observed, &mut count);
    complete(&mut scheduler, 0, 0);
    step(&mut scheduler, 0);
    request(&mut scheduler, 0, 1);
    control(&mut scheduler, 0, 1);
    step(&mut scheduler, 0);
    assert_eq!(scheduler.undispatched_cancellation_count, 2);
    // Slot one is still queued. The fresh slot-zero request must take precedence.
    if cancel_all {
        scheduler.cancel().unwrap();
        assert!(scheduler.next_host_cancellation().is_none());
        assert!(scheduler.next_host_request().is_none());
        assert_eq!(scheduler.step(), Ok(SchedulerStatus::Cancelled));
        assert!(scheduler.drivers.iter().all(|driver| driver.cancelled));
    } else {
        if let Some(node) = settled {
            complete(&mut scheduler, node, if node == 0 { 1 } else { 0 });
        }
        if settled != Some(0) {
            poll(&mut scheduler, 0, 1, &mut observed, &mut count);
        }
        if settled != Some(1) {
            poll(&mut scheduler, 1, 0, &mut observed, &mut count);
        }
        assert!(scheduler.next_host_cancellation().is_none());
        assert_eq!(scheduler.undispatched_cancellation_count, 0);
        if settled != Some(0) {
            complete(&mut scheduler, 0, 1);
        }
        if settled != Some(1) {
            complete(&mut scheduler, 1, 0);
        }
        step(&mut scheduler, 1);
        request(&mut scheduler, 1, 1);
        step(&mut scheduler, 0);
        control(&mut scheduler, 1, 1);
        step(&mut scheduler, 1);
        poll(&mut scheduler, 1, 1, &mut observed, &mut count);
        complete(&mut scheduler, 1, 1);
        assert_eq!(scheduler.step(), Ok(SchedulerStatus::Drained));
        scheduler.discard_host_value(input).unwrap();
        assert!(scheduler.drivers.iter().all(|driver| driver.phase == 5));
        assert_eq!(scheduler.values.access().stored_bytes, 4);
        assert_eq!(scheduler.signs.events().count(), 32);
    }
    assert_eq!(scheduler.pending_host_call_count(), 0);
    assert!(scheduler.pending_host_slots.iter().all(Option::is_none));
    assert!(!scheduler.has_retained_host_call_for_nodes(&[NodeId(0), NodeId(1)]));
    assert_eq!(scheduler.values.used_items(), 0);
    assert_eq!(scheduler.values.used_bytes(), 0);
    assert!(!scheduler.has_ready_work());
    assert!(scheduler.signs.retention_gap.is_none());
    let settled = settled.map(i32::from).unwrap_or(-1);
    std::println!("CANCEL_INTERLEAVING_ROW {{\"settled\":{settled},\"cancel_all\":{cancel_all},\"polls\":{observed:?}}}");
}
#[test]
fn earlier_cancellation_reinsertion_preserves_order_through_completion_races() {
    for settled in [None, Some(0), Some(1)] {
        interleave(settled, false);
    }
}
#[test]
fn global_cancel_clears_reinserted_and_queued_cancellations() {
    interleave(None, true);
}

// Compare adapter-visible behavior with the original full-table traversal,
// including stateful predicates whose result depends on callback order.
fn compare_adapter(scheduler: &mut Pair, mask: u8, alternating: bool) {
    let cursor = scheduler.host_request_cursor;
    let mut expected_calls = std::vec::Vec::new();
    let mut expected = None;
    for offset in 0..4 {
        let slot = (cursor + offset) % 4;
        let Some(pending) = scheduler.pending_host_calls[slot] else {
            continue;
        };
        if pending.dispatched {
            continue;
        }
        let request = pending.request;
        expected_calls.push((request.node, request.request));
        let accepted =
            mask & (1 << request.node.0) != 0 && (!alternating || expected_calls.len() % 2 == 0);
        if accepted {
            expected = Some((slot, request));
            break;
        }
    }
    let signs = scheduler.signs.entries;
    let before = scheduler.undispatched_host_count;
    let mut actual_calls = std::vec::Vec::new();
    let actual = scheduler.next_host_request_matching(|request| {
        actual_calls.push((request.node, request.request));
        mask & (1 << request.node.0) != 0 && (!alternating || actual_calls.len() % 2 == 0)
    });
    assert_eq!(actual_calls, expected_calls);
    assert_eq!(actual, expected.map(|(_, request)| request));
    assert_eq!(scheduler.signs.entries, signs);
    assert_eq!(
        scheduler.undispatched_host_count,
        before - u32::from(expected.is_some())
    );
    assert_eq!(
        scheduler.host_request_cursor,
        expected.map_or(cursor, |(slot, _)| (slot + 1) % 4)
    );
    assert_eq!(
        scheduler.undispatched_host_count as usize,
        scheduler
            .pending_host_calls
            .iter()
            .flatten()
            .filter(|p| !p.dispatched)
            .count()
    );
}

#[test]
fn adapter_selection_matches_full_scan_through_competition_completion_and_reuse() {
    for cursor in 0..4 {
        for mask in 0..4 {
            for alternating in [false, true] {
                for first in 0..2 {
                    let (mut scheduler, _) = pair();
                    step(&mut scheduler, 0);
                    step(&mut scheduler, 1);
                    scheduler.host_request_cursor = cursor;
                    compare_adapter(&mut scheduler, mask, alternating);
                    // A competing adapter sees precisely the remaining requests.
                    compare_adapter(&mut scheduler, mask ^ 3, alternating);
                    compare_adapter(&mut scheduler, 3, false);
                    compare_adapter(&mut scheduler, 3, false);
                    assert_eq!(scheduler.undispatched_host_count, 0);
                    for node in 0..2 {
                        control(&mut scheduler, node, 0);
                        step(&mut scheduler, node);
                    }
                    complete(&mut scheduler, first, 0);
                    step(&mut scheduler, first);
                    // The completed slot is reused while the other remains pending.
                    assert_eq!(scheduler.undispatched_host_count, 1);
                    compare_adapter(&mut scheduler, mask, alternating);
                    compare_adapter(&mut scheduler, 3, false);
                    complete(&mut scheduler, 1 - first, 0);
                    step(&mut scheduler, 1 - first);
                    compare_adapter(&mut scheduler, mask ^ 3, alternating);
                    compare_adapter(&mut scheduler, 3, false);
                    scheduler.cancel().unwrap();
                    compare_adapter(&mut scheduler, 3, false);
                    assert_eq!(scheduler.values.used_items(), 0);
                }
            }
        }
    }
}

#[test]
fn completion_sign_refusal_preserves_queued_cancellation_until_accepted_retry() {
    for cancellation_first in [false, true] {
        let (mut scheduler, _) = pair();
        for node in 0..2 {
            step(&mut scheduler, node);
            request(&mut scheduler, node, 0);
            control(&mut scheduler, node, 0);
        }
        step(&mut scheduler, 0);
        step(&mut scheduler, 1);
        assert_eq!(scheduler.undispatched_cancellation_count, 2);
        assert!(!scheduler.has_ready_work());
        let sequence = scheduler.signs.next_sequence;
        let signs = scheduler.signs.entries;
        let used_items = scheduler.values.used_items();
        let slots = scheduler.pending_host_slots;
        // Fault injection: no next Sign identity can be assigned. Admission
        // must refuse before changing completion, readiness, or cancellation.
        scheduler.signs.next_sequence = u32::MAX;
        for _ in 0..2 {
            assert_eq!(
                scheduler.complete_host_call(
                    NodeId(0),
                    RequestId(0),
                    HostCallOutcome {
                        disposition: HostCallDisposition::Cancelled,
                        output: None,
                        failure: None,
                    }
                ),
                Err(SchedulerError::Sign(crate::SignError::SequenceOverflow))
            );
            assert_eq!(scheduler.undispatched_cancellation_count, 2);
            assert_eq!(scheduler.pending_host_slots, slots);
            assert_eq!(scheduler.values.used_items(), used_items);
            assert_eq!(scheduler.signs.entries, signs);
            assert!(!scheduler.has_ready_work());
            for pending in scheduler.pending_host_calls.iter().flatten() {
                assert!(pending.completion.is_none());
                assert!(pending.cancellation_requested);
                assert!(!pending.cancellation_dispatched);
            }
        }
        scheduler.signs.next_sequence = sequence;
        let mut observed = [u32::MAX; 4];
        let mut count = 0;
        if cancellation_first {
            poll(&mut scheduler, 0, 0, &mut observed, &mut count);
        }
        complete(&mut scheduler, 0, 0);
        assert_eq!(scheduler.undispatched_cancellation_count, 1);
        poll(&mut scheduler, 1, 0, &mut observed, &mut count);
        assert!(scheduler.next_host_cancellation().is_none());
        complete(&mut scheduler, 1, 0);
        assert_eq!(scheduler.undispatched_cancellation_count, 0);
        assert_eq!(count, if cancellation_first { 2 } else { 1 });
        assert_eq!(observed[count - 1], 10);
        step(&mut scheduler, 0);
        step(&mut scheduler, 1);
        request(&mut scheduler, 0, 1);
        request(&mut scheduler, 1, 1);
        scheduler.cancel().unwrap();
        assert_eq!(scheduler.pending_host_call_count(), 0);
        assert_eq!(scheduler.values.used_items(), 0);
    }
}
