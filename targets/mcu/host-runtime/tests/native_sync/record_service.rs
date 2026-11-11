//! Real two-call owner and real kernel: no invented second residence/provider.
use super::*;
use conduit_resource_snapshot::read_result::*;
#[path = "service_fixture.rs"]
mod service_fixture;
struct Back {
    input: [ValueRef; 2],
    calls: [HostCallId; 2],
    phase: u8,
}
impl StepBack<1> for Back {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        if self.phase % 2 == 1 {
            if let Some((request, outcome)) = io.host_completion() {
                assert_eq!(request, RequestId(u32::from(self.phase / 2)));
                assert_eq!(outcome.disposition, HostCallDisposition::Completed);
                if self.phase == 3 {
                    let result = decode(bytes.host_output().unwrap()).unwrap();
                    assert!(matches!(
                        result,
                        ReadResult::Recorded {
                            index: 0,
                            record: b"sample",
                            ..
                        }
                    ));
                }
                io.consume_host_completion().unwrap();
                self.phase += 1;
            } else {
                return StepOutcome::Await;
            }
        }
        if self.phase == 4 {
            return StepOutcome::Complete;
        }
        let index = usize::from(self.phase / 2);
        io.request_host_call(
            RequestId(index as u32),
            self.calls[index],
            BoundedValueRef::new(self.input[index], if index == 0 { 200 } else { 8 }).unwrap(),
        )
        .unwrap();
        self.phase += 1;
        StepOutcome::Progress
    }
}
type ServiceKernel = StoredExecutionKernel<Back, Signs, 1, 1, 1, 0, 0, 3>;
fn kernel(calls: [HostCallId; 2], value_bytes: u32, fail: Rc<Cell<bool>>) -> ServiceKernel {
    let mut values = HostedValueStore::new(8, READ_RESULT_BYTES as u32, value_bytes).unwrap();
    let sample = values.store(b"sample").unwrap();
    let count = conduit_core::encode_count(0);
    let index = values.store(&count).unwrap();
    let mut routes = FixedRoutes::<0, 0>::new(1);
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<3>::new(3);
    for call in [HostCallId(0), HostCallId(1), HostCallId(2)] {
        bindings
            .install(
                NodeId(0),
                HostCallBinding {
                    call,
                    maximum_input_bytes: 200,
                    maximum_output_bytes: if call == HostCallId(1) {
                        READ_RESULT_BYTES as u32
                    } else {
                        APPEND_RESULT_BYTES
                    },
                },
            )
            .unwrap();
    }
    bindings.seal().unwrap();
    ServiceKernel::new_with_active_counts_and_host_calls(
        1,
        0,
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 8,
        }],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 200,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        bindings,
        [Back {
            input: [sample, index],
            calls,
            phase: 0,
        }],
        values,
        Signs {
            log: FixedSignLog::new(64 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
            fail,
        },
    )
    .unwrap()
}
#[test]
fn one_actual_owner_services_append_then_read_with_exact_kernel_acceptance() {
    let storage = fixture::Storage::new();
    let mut owner = service_fixture::bound(storage.clone());
    let append = owner.routes()[0].call;
    let read = owner.routes()[1].call;
    let mut kernel = kernel([append, read], 2000, Rc::new(Cell::new(false)));
    let activity = NativeActivity::new();
    let mut output = [0; READ_RESULT_BYTES];
    let mut service = Service::new(&mut owner, &mut output).unwrap();
    kernel.step().unwrap();
    assert!(matches!(
        allocations::without_allocations(|| service.dispatch(&mut kernel, &activity)),
        Ok(Dispatch::Completed { .. })
    ));
    assert!(service.bound_owner().custody().unwrap().kernel_committed);
    assert_eq!(storage.programs.get(), 2);
    kernel.step().unwrap();
    assert!(matches!(
        allocations::without_allocations(|| service.dispatch(&mut kernel, &activity)),
        Ok(Dispatch::Completed { .. })
    ));
    assert!(service.bound_owner().pending_outcome().unwrap().3);
    assert_eq!(
        service
            .bound_owner()
            .inspection(0)
            .unwrap()
            .completed_operations,
        1
    );
    assert_eq!(
        service
            .bound_owner()
            .inspection(1)
            .unwrap()
            .completed_operations,
        1
    );
    kernel.step().unwrap();
    assert_eq!(storage.programs.get(), 2);
    service.revoke().unwrap();
}
#[test]
fn unknown_call_is_not_taken_and_pressure_preserves_original_request() {
    let storage = fixture::Storage::new();
    let mut owner = service_fixture::bound(storage.clone());
    let append = owner.routes()[0].call;
    let read = owner.routes()[1].call;
    let activity = NativeActivity::new();
    let mut output = [0; READ_RESULT_BYTES];
    let mut service = Service::new(&mut owner, &mut output).unwrap();
    let mut unknown = kernel([HostCallId(2), read], 2000, Rc::new(Cell::new(false)));
    unknown.step().unwrap();
    assert_eq!(
        allocations::without_allocations(|| service.dispatch(&mut unknown, &activity)),
        Ok(Dispatch::Idle)
    );
    assert_eq!(unknown.next_host_request().unwrap().call, HostCallId(2));
    let fail = Rc::new(Cell::new(false));
    let mut pending = kernel([append, read], 2000, fail.clone());
    pending.step().unwrap();
    fail.set(true);
    assert_eq!(
        allocations::without_allocations(|| service.dispatch(&mut pending, &activity)),
        Ok(Dispatch::Pressure)
    );
    assert_eq!(storage.programs.get(), 0);
    assert!(service.bound_owner().pending_outcome().is_none());
    fail.set(false);
    assert!(matches!(
        allocations::without_allocations(|| service.dispatch(&mut pending, &activity)),
        Ok(Dispatch::Completed { .. })
    ));
    assert_eq!(storage.programs.get(), 2);
    service.revoke().unwrap();
}

#[test]
fn append_uses_its_smaller_result_bound_and_read_pressure_keeps_same_request() {
    let storage = fixture::Storage::new();
    let mut owner = service_fixture::bound(storage.clone());
    let append = owner.routes()[0].call;
    let read = owner.routes()[1].call;
    let mut kernel = kernel(
        [append, read],
        APPEND_RESULT_BYTES + 14,
        Rc::new(Cell::new(false)),
    );
    let activity = NativeActivity::new();
    let mut output = [0; READ_RESULT_BYTES];
    let mut service = Service::new(&mut owner, &mut output).unwrap();
    kernel.step().unwrap();
    assert!(matches!(
        allocations::without_allocations(|| service.dispatch(&mut kernel, &activity)),
        Ok(Dispatch::Completed { .. })
    ));
    kernel.step().unwrap();
    assert_eq!(
        allocations::without_allocations(|| service.dispatch(&mut kernel, &activity)),
        Ok(Dispatch::Pressure)
    );
    assert_eq!(storage.programs.get(), 2);
    let still_pending = kernel.next_host_request().unwrap();
    assert_eq!(still_pending.call, read);
    assert_eq!(still_pending.request, RequestId(1));
    assert_eq!(
        service
            .bound_owner()
            .inspection(1)
            .unwrap()
            .completed_operations,
        0
    );
    service.revoke().unwrap();
}
#[test]
fn rejected_kernel_completion_retains_unacknowledged_service_custody() {
    let storage = fixture::Storage::new();
    let inner = service_fixture::bound(storage.clone());
    let append = inner.routes()[0].call;
    let read = inner.routes()[1].call;
    let fail = Rc::new(Cell::new(false));
    let mut owner = Interrupt {
        inner,
        fail: fail.clone(),
    };
    let mut kernel = kernel([append, read], 2000, fail.clone());
    let activity = NativeActivity::new();
    let mut output = [0; READ_RESULT_BYTES];
    let mut service = Service::new(&mut owner, &mut output).unwrap();
    kernel.step().unwrap();
    assert!(matches!(
        allocations::without_allocations(|| service.dispatch(&mut kernel, &activity)),
        Err(Refusal::Kernel(SchedulerError::Sign(_)))
    ));
    assert_eq!(storage.programs.get(), 2);
    assert!(
        !service
            .bound_owner()
            .inner
            .custody()
            .unwrap()
            .kernel_committed
    );
    assert!(!service.bound_owner().inner.pending_outcome().unwrap().3);
    assert_eq!(
        allocations::without_allocations(|| service.dispatch(&mut kernel, &activity)),
        Ok(Dispatch::Idle)
    );
    service.revoke().unwrap();
    assert!(
        !service
            .bound_owner()
            .inner
            .custody()
            .unwrap()
            .kernel_committed
    );
}

#[path = "factory_service.rs"]
mod factory_service;
