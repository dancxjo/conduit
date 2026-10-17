//! Distinct indexed Host slots survive out-of-order completion.
use super::*;

struct IndexedBack {
    input: ValueRef,
    requested: bool,
    completed: bool,
}
impl StepBack<PORTS> for IndexedBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if !self.requested {
            io.request_host_call(
                RequestId(7),
                HostCallId(0),
                BoundedValueRef::new(self.input, 0).unwrap(),
            )
            .unwrap();
            self.requested = true;
            return StepOutcome::Progress;
        }
        if let Some((request, _)) = io.host_completion() {
            assert_eq!(request, RequestId(7));
            io.consume_host_completion().unwrap();
            self.completed = true;
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
}

#[test]
fn distinct_nodes_keep_exact_slots_through_out_of_order_completion() {
    let mut values = FixedValueStore::<1, 1>::new(1).unwrap();
    let input = values.store(b"").unwrap();
    let mut routes = FixedRoutes::<0, 0>::new(PORTS as u16);
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<2>::new(1);
    for node in [NodeId(0), NodeId(1)] {
        bindings
            .install(
                node,
                HostCallBinding {
                    call: HostCallId(0),
                    maximum_input_bytes: 0,
                    maximum_output_bytes: 0,
                },
            )
            .unwrap();
    }
    bindings.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 2, 0, PORTS, 0, 0, 0, 2, 2>::new_with_host_calls(
        [node([None; PORTS]), node([None; PORTS])],
        [],
        routes,
        bindings,
        [
            IndexedBack {
                input,
                requested: false,
                completed: false,
            },
            IndexedBack {
                input,
                requested: false,
                completed: false,
            },
        ],
        values,
        FixedSignLog::<32>::new(32 * core::mem::size_of::<crate::KernelEvent>() as u32).unwrap(),
    )
    .unwrap();
    scheduler.step().unwrap();
    let first = scheduler.next_host_request().unwrap();
    scheduler.step().unwrap();
    let second = scheduler.next_host_request().unwrap();
    assert_eq!((first.node, second.node), (NodeId(0), NodeId(1)));
    assert_eq!(
        scheduler
            .pending_host_slots
            .map(|slot| slot.map(|slot| slot.get())),
        [Some(1), Some(2)]
    );
    let outcome = HostCallOutcome {
        disposition: HostCallDisposition::Completed,
        output: None,
        failure: None,
    };
    assert_eq!(
        scheduler.complete_host_call(first.node, RequestId(99), outcome),
        Err(SchedulerError::HostCallCompletionRejected)
    );
    scheduler
        .complete_host_call(second.node, second.request, outcome)
        .unwrap();
    assert_eq!(
        scheduler.step(),
        Ok(SchedulerStatus::Progress { node: NodeId(1) })
    );
    assert!(!scheduler.drivers[0].completed && scheduler.drivers[1].completed);
    assert_eq!(
        scheduler
            .pending_host_slots
            .map(|slot| slot.map(|slot| slot.get())),
        [Some(1), None]
    );
    assert!(!scheduler.has_ready_work());
    scheduler
        .complete_host_call(first.node, first.request, outcome)
        .unwrap();
    scheduler.step().unwrap();
    assert!(scheduler.drivers.iter().all(|driver| driver.completed));
    assert_eq!(scheduler.pending_host_call_count(), 0);
    assert!(scheduler.pending_host_slots.iter().all(Option::is_none));
}
