//! Preserve first-free slot identities without quadratic initial filling.
use super::*;
struct Requester {
    input: ValueRef,
    requested: bool,
}
impl StepBack<PORTS> for Requester {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let request = if !self.requested {
            self.requested = true;
            RequestId(0)
        } else if io.host_completion().is_some() {
            io.consume_host_completion().unwrap();
            RequestId(1)
        } else {
            return StepOutcome::Await;
        };
        io.request_host_call(
            request,
            HostCallId(0),
            BoundedValueRef::new(self.input, 0).unwrap(),
        )
        .unwrap();
        StepOutcome::Progress
    }
}
#[test]
fn host_slot_fill_is_linear_and_out_of_order_replacement_keeps_first_free_order() {
    const N: usize = 64;
    let mut values = FixedValueStore::<1, 1>::new(1).unwrap();
    let input = values.store(b"").unwrap();
    let mut routes = FixedRoutes::<0, 0>::new(PORTS as u16);
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<N>::new(1);
    for node in 0..N {
        bindings
            .install(
                NodeId(node as u16),
                HostCallBinding {
                    call: HostCallId(0),
                    maximum_input_bytes: 0,
                    maximum_output_bytes: 0,
                },
            )
            .unwrap();
    }
    bindings.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, N, 0, PORTS, 1, 0, 0, N, N>::new_with_active_counts_and_host_calls(
        N, 0, core::array::from_fn(|_| node([None; PORTS])), [], routes, bindings,
        core::array::from_fn(|_| Requester { input, requested: false }), values,
        FixedSignLog::<256>::new(256 * core::mem::size_of::<crate::KernelEvent>() as u32).unwrap(),
    ).unwrap();
    for node in 0..N {
        assert_eq!(
            scheduler.step(),
            Ok(SchedulerStatus::Progress {
                node: NodeId(node as u16)
            })
        );
        assert_eq!(
            scheduler.next_host_request().unwrap().node,
            NodeId(node as u16)
        );
        assert_eq!(scheduler.indexed_host_slot(node), Some(node));
    }
    assert_eq!(scheduler.pending_host_call_count(), N);
    assert_eq!(
        scheduler.scan_counts.get().allocation_pending_slots,
        N as u64
    );
    for node in [33, 5] {
        scheduler
            .complete_host_call(
                NodeId(node),
                RequestId(0),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .unwrap();
    }
    for node in [5, 33] {
        assert_eq!(
            scheduler.step(),
            Ok(SchedulerStatus::Progress { node: NodeId(node) })
        );
        assert_eq!(
            scheduler.indexed_host_slot(usize::from(node)),
            Some(usize::from(node))
        );
        let request = scheduler.next_host_request().unwrap();
        assert_eq!(
            (request.node, request.request),
            (NodeId(node), RequestId(1))
        );
        assert_eq!(scheduler.pending_host_call_count(), N);
    }
    scheduler.cancel().unwrap();
    assert_eq!(scheduler.pending_host_call_count(), 0);
}
