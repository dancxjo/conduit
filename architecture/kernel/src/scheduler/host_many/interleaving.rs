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
