//! Pending ownership inspection stops after the last occupied slot, including holes.
extern crate std;
use super::*;
use crate::scheduler::HostValueDiscardRefusal;

struct OwnerBack {
    input: ValueRef,
    requested: bool,
}
impl StepBack<PORTS> for OwnerBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if !self.requested {
            io.request_host_call(
                RequestId(1),
                HostCallId(0),
                BoundedValueRef::new(self.input, 8).unwrap(),
            )
            .unwrap();
            self.requested = true;
            StepOutcome::Progress
        } else if io.host_completion().is_some() {
            io.consume_host_completion().unwrap();
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
}

type Owner = FixedScheduler<
    OwnerBack,
    FixedValueStore<8, 8>,
    FixedSignLog<64>,
    2,
    0,
    PORTS,
    1,
    0,
    0,
    2,
    4096,
>;
fn scheduler() -> Owner {
    let mut values = FixedValueStore::new(64).unwrap();
    let inputs = [values.store(b"a").unwrap(), values.store(b"b").unwrap()];
    let mut routes = FixedRoutes::new(PORTS as u16);
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::new(1);
    for node in [NodeId(0), NodeId(1)] {
        bindings
            .install(
                node,
                HostCallBinding {
                    call: HostCallId(0),
                    maximum_input_bytes: 8,
                    maximum_output_bytes: 8,
                },
            )
            .unwrap();
    }
    bindings.seal().unwrap();
    Owner::new_with_active_counts_and_host_calls(
        2,
        0,
        [node([None; PORTS]), node([None; PORTS])],
        [],
        routes,
        bindings,
        inputs.map(|input| OwnerBack {
            input,
            requested: false,
        }),
        values,
        FixedSignLog::new(64 * core::mem::size_of::<crate::KernelEvent>() as u32).unwrap(),
    )
    .unwrap()
}

fn temporary_probe_count(scheduler: &mut Owner) -> u64 {
    let value = scheduler.store_host_value(b"temp").unwrap();
    scheduler.scan_counts.set(Default::default());
    scheduler.discard_host_value(value).unwrap();
    scheduler.scan_counts.get().value_owner_pending_slots
}

#[test]
fn pending_ownership_skips_unused_tail_without_skipping_holes_or_completion_outputs() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut scheduler = scheduler();
            scheduler.step().unwrap();
            let first = scheduler.next_host_request().unwrap();
            assert_eq!(temporary_probe_count(&mut scheduler), 1);
            scheduler.step().unwrap();
            let second = scheduler.next_host_request().unwrap();
            assert_eq!(temporary_probe_count(&mut scheduler), 2);
            for request in [first, second] {
                assert_eq!(
                    scheduler.discard_host_value(request.input.value),
                    Err(HostValueDiscardRefusal::ValueOwned)
                );
            }
            scheduler
                .complete_host_call(
                    first.node,
                    first.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .unwrap();
            scheduler.step().unwrap();
            assert!(scheduler.pending_host_calls[0].is_none());
            assert!(scheduler.pending_host_calls[1].is_some());
            assert_eq!(scheduler.pending_host_call_count(), 1);
            assert_eq!(temporary_probe_count(&mut scheduler), 2);
            let output = scheduler.store_host_value(b"out").unwrap();
            scheduler
                .complete_host_call(
                    second.node,
                    second.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(BoundedValueRef::new(output, 8).unwrap()),
                        failure: None,
                    },
                )
                .unwrap();
            for value in [second.input.value, output] {
                assert_eq!(
                    scheduler.discard_host_value(value),
                    Err(HostValueDiscardRefusal::ValueOwned)
                );
            }
            assert_eq!(temporary_probe_count(&mut scheduler), 2);
            scheduler.step().unwrap();
            assert_eq!(scheduler.pending_host_call_count(), 0);
            assert_eq!(temporary_probe_count(&mut scheduler), 0);
            assert_eq!(scheduler.values().used_items(), 0);
        })
        .unwrap()
        .join()
        .unwrap();
}
