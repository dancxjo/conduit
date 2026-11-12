//! Real bound native owners and the actual kernel; CPU providers, no physical
//! attachment, whole Body admission or durable recording claim.
use super::*;
use conduit_core as c;
use conduit_i2c_transaction::{
    contract::{I2cContract, I2C_CALL, I2C_MAXIMUM_BYTES, I2C_RESULT_VALUE_BYTES},
    owner::{I2cAttachment, I2cCallSelection, I2cHostCall},
    I2cDisposition, I2cProvider, I2cTransaction,
};
use conduit_mcu_host_runtime::native_dispatch::deadline;
use conduit_monotonic_wait::clock_at::{
    codec::ClockDisposition,
    contract::{MonotonicClockContract, CLOCK_CALL, CLOCK_MAXIMUM_BYTES},
    owner::{
        ClockCallSelection, MonotonicClockHostCall, MonotonicDeadlineProvider, MAXIMUM_POLL_STEPS,
    },
};

fn scope(kind: &c::Kind, call: &str, maximum: u32, work: u64) -> c::BaseCapabilityScope {
    let mut s = fixture::scope();
    s.base_instance_id = format!("fixture/{}", kind.kind_id.as_str()).into();
    s.resource_pool_id = format!("pool/{}", kind.kind_id.as_str()).into();
    s.subject_kind = kind.kind_id.clone();
    s.operation_contract_id = call.into();
    s.maximum_parameter_bytes = maximum;
    s.maximum_result_bytes = maximum;
    s.maximum_work_units = work;
    s.maximum_operations = 2;
    s
}
struct Bus {
    effects: Rc<Cell<usize>>,
    revoked: Rc<Cell<bool>>,
}
impl I2cProvider for Bus {
    fn transact(
        &mut self,
        request: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        assert!(!self.revoked.get());
        self.effects.set(self.effects.get() + 1);
        let n = usize::from(request.read_length());
        input[..n].fill(0x60);
        Ok(n)
    }
    fn revoke(&mut self) {
        self.revoked.set(true);
    }
}
fn bus(effects: Rc<Cell<usize>>, revoked: Rc<Cell<bool>>) -> I2cHostCall<Bus> {
    let contract = I2cContract::prepare().unwrap();
    let s = scope(contract.kind(), I2C_CALL, I2C_MAXIMUM_BYTES, 1);
    let (fragment, lowered, active, placement) = fixture::selection_fixture::selected(
        &s,
        contract.kind(),
        I2C_CALL,
        I2C_MAXIMUM_BYTES,
        "i2c",
        "i2c",
    );
    bind_bus(
        s,
        Bus { effects, revoked },
        I2cCallSelection {
            contract: &contract,
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement: &placement,
        },
    )
}
fn bind_bus(
    s: c::BaseCapabilityScope,
    provider: Bus,
    selection: I2cCallSelection<'_>,
) -> I2cHostCall<Bus> {
    let (table, handle, claim) = fixture::authorization_with_bounds(
        s,
        selection.fragment,
        selection.active,
        I2C_MAXIMUM_BYTES,
        1,
    );
    // SAFETY: exclusively held finite synchronous CPU provider and exact fixture
    // attachment. The original selected Plan is checked by the native binder.
    unsafe {
        I2cHostCall::bind_selected(
            table,
            handle,
            claim,
            I2cAttachment {
                generation: 1,
                minimum_address: 0x70,
                maximum_address: 0x76,
                resource_bytes: 4096,
            },
            provider,
            selection,
        )
    }
    .unwrap()
}

fn bus_request(address: u8) -> Vec<u8> {
    let ty = I2cContract::prepare()
        .unwrap()
        .decode_request_type()
        .unwrap();
    let c::StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        unreachable!()
    };
    let field = |name| {
        fields
            .iter()
            .find(|f| f.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    let octet = c::StructuredInfoType::leaf(c::kind_id("value/u8")).unwrap();
    c::StructuredInfoValue::record(
        ty.clone(),
        vec![
            c::StructuredFieldValue::new(
                "address",
                c::StructuredInfoValue::leaf(field("address"), vec![address]).unwrap(),
            )
            .unwrap(),
            c::StructuredFieldValue::new(
                "read",
                c::StructuredInfoValue::leaf(field("read"), vec![1]).unwrap(),
            )
            .unwrap(),
            c::StructuredFieldValue::new(
                "write",
                c::StructuredInfoValue::sequence(
                    field("write"),
                    vec![c::StructuredInfoValue::leaf(octet, vec![0xd0]).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
struct Clock {
    polls: Rc<Cell<usize>>,
    next: Rc<Cell<Option<u64>>>,
    revoked: Rc<Cell<bool>>,
}
impl MonotonicDeadlineProvider for Clock {
    fn poll_until(&mut self, _: u64) -> Result<Option<u64>, ClockDisposition> {
        assert!(!self.revoked.get());
        self.polls.set(self.polls.get() + 1);
        Ok(self.next.take())
    }
    fn revoke(&mut self) {
        self.revoked.set(true);
    }
}
fn clock(
    polls: Rc<Cell<usize>>,
    next: Rc<Cell<Option<u64>>>,
    revoked: Rc<Cell<bool>>,
) -> MonotonicClockHostCall<Clock> {
    let contract = MonotonicClockContract::prepare().unwrap();
    let s = scope(
        contract.kind(),
        CLOCK_CALL,
        CLOCK_MAXIMUM_BYTES,
        MAXIMUM_POLL_STEPS,
    );
    let (fragment, lowered, active, placement) = fixture::selection_fixture::selected(
        &s,
        contract.kind(),
        CLOCK_CALL,
        CLOCK_MAXIMUM_BYTES,
        "clock",
        "clock",
    );
    bind_clock(
        s,
        Clock {
            polls,
            next,
            revoked,
        },
        ClockCallSelection {
            contract: &contract,
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement: &placement,
        },
    )
}
fn bind_clock(
    s: c::BaseCapabilityScope,
    provider: Clock,
    selection: ClockCallSelection<'_>,
) -> MonotonicClockHostCall<Clock> {
    let (table, handle, claim) = fixture::authorization_with_bounds(
        s,
        selection.fragment,
        selection.active,
        CLOCK_MAXIMUM_BYTES,
        MAXIMUM_POLL_STEPS,
    );
    // SAFETY: exclusive bounded CPU clock, exact independently issued scope;
    // polls never block or allocate and revocation quiesces future observations.
    unsafe { MonotonicClockHostCall::bind_selected(table, handle, claim, provider, selection) }
        .unwrap()
}

fn clock_request() -> Vec<u8> {
    let ty = MonotonicClockContract::prepare()
        .unwrap()
        .request_type()
        .clone();
    let c::StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        unreachable!()
    };
    let instant =
        c::StructuredInfoValue::leaf(fields[0].value_type().clone(), 20u64.to_le_bytes().to_vec())
            .unwrap();
    c::StructuredInfoValue::record(
        ty,
        vec![c::StructuredFieldValue::new("deadline", instant).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
struct Request {
    input: ValueRef,
    maximum: u32,
    tag: &'static str,
    started: bool,
}
impl StepBack<1> for Request {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        if !self.started {
            io.request_host_call(
                RequestId(0),
                HostCallId(0),
                BoundedValueRef::new(self.input, self.maximum).unwrap(),
            )
            .unwrap();
            self.started = true;
            return StepOutcome::Progress;
        }
        if let Some((_, result)) = io.host_completion() {
            assert_eq!(result.disposition, HostCallDisposition::Completed);
            let value =
                c::validate_canonical_structured_value(bytes.host_output().unwrap()).unwrap();
            assert_eq!(value.variant_tag().unwrap(), self.tag);
            io.consume_host_completion().unwrap();
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}
fn request_kernel(
    input: &[u8],
    maximum: u32,
    result: u32,
    value_bytes: u32,
    tag: &'static str,
    fail: Rc<Cell<bool>>,
) -> StoredExecutionKernel<Request, Signs, 1, 1, 1, 0, 0, 1> {
    let mut values = HostedValueStore::new(4, I2C_MAXIMUM_BYTES, value_bytes).unwrap();
    let input = values.store(input).unwrap();
    kernel_with_back(
        values,
        Request {
            input,
            maximum,
            tag,
            started: false,
        },
        maximum,
        result,
        fail,
    )
}
#[test]
fn i2c_preflight_preserves_original_request_then_completes_once_without_allocations() {
    for address in [0x76, 0x50] {
        let effects = Rc::new(Cell::new(0));
        let revoked = Rc::new(Cell::new(false));
        let mut owner = bus(effects.clone(), revoked.clone());
        let input = bus_request(address);
        let fail = Rc::new(Cell::new(false));
        let mut kernel = request_kernel(
            &input,
            I2C_MAXIMUM_BYTES,
            I2C_RESULT_VALUE_BYTES as u32,
            8192,
            if address == 0x76 {
                "completed"
            } else {
                "refused"
            },
            fail.clone(),
        );
        kernel.step().unwrap();
        let mut output = [0; I2C_RESULT_VALUE_BYTES];
        let activity = NativeActivity::new();
        let mut service = Service::new(&mut owner, &mut output).unwrap();
        fail.set(true);
        allocations::without_allocations(|| {
            for _ in 0..8 {
                assert_eq!(
                    service.dispatch(&mut kernel, &activity),
                    Ok(Dispatch::Pressure)
                );
            }
        });
        assert_eq!(effects.get(), 0);
        fail.set(false);
        let timing = activity.try_timing().unwrap();
        assert_eq!(service.dispatch(&mut kernel, &activity), Ok(Dispatch::Idle));
        drop(timing);
        allocations::without_allocations(|| {
            assert!(matches!(
                service.dispatch(&mut kernel, &activity),
                Ok(Dispatch::Completed { .. })
            ));
            assert_eq!(service.dispatch(&mut kernel, &activity), Ok(Dispatch::Idle));
            kernel.step().unwrap();
        });
        assert_eq!(effects.get(), usize::from(address == 0x76));
        assert_eq!(service.bound_owner().inspection().in_flight_operations, 0);
        service.revoke().unwrap();
        assert!(revoked.get());
    }
}
#[test]
fn i2c_value_pressure_and_short_buffer_refuse_before_physical_effects() {
    let effects = Rc::new(Cell::new(0));
    let mut owner = bus(effects.clone(), Rc::new(Cell::new(false)));
    let input = bus_request(0x76);
    let mut kernel = request_kernel(
        &input,
        I2C_MAXIMUM_BYTES,
        I2C_RESULT_VALUE_BYTES as u32,
        input.len() as u32,
        "completed",
        Rc::new(Cell::new(false)),
    );
    kernel.step().unwrap();
    let mut short = [0; I2C_RESULT_VALUE_BYTES - 1];
    assert!(matches!(
        Service::new(&mut owner, &mut short),
        Err(Refusal::InvalidBounds)
    ));
    let mut output = [0; I2C_RESULT_VALUE_BYTES];
    let mut service = Service::new(&mut owner, &mut output).unwrap();
    assert_eq!(
        service.dispatch(&mut kernel, &NativeActivity::new()),
        Ok(Dispatch::Pressure)
    );
    assert_eq!(effects.get(), 0);
    service.revoke().unwrap();
}
#[test]
fn deadline_pending_pressure_completion_and_revocation_keep_exact_lease() {
    for cancel in [false, true] {
        let polls = Rc::new(Cell::new(0));
        let next = Rc::new(Cell::new(None));
        let revoked = Rc::new(Cell::new(false));
        let mut owner = clock(polls.clone(), next.clone(), revoked.clone());
        let fail = Rc::new(Cell::new(false));
        let mut kernel = request_kernel(
            &clock_request(),
            CLOCK_MAXIMUM_BYTES,
            CLOCK_MAXIMUM_BYTES,
            4096,
            "completed",
            fail.clone(),
        );
        kernel.step().unwrap();
        let mut service = deadline::Service::new(&mut owner);
        allocations::without_allocations(|| {
            assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Idle))
        });
        assert_eq!(polls.get(), 1);
        assert_eq!(service.bound_owner().inspection().in_flight_operations, 1);
        fail.set(true);
        assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Pressure));
        assert_eq!(polls.get(), 1);
        next.set(Some(23));
        fail.set(false);
        if cancel {
            service.revoke().unwrap();
            kernel.cancel().unwrap();
            assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Idle));
            assert_eq!(polls.get(), 1);
        } else {
            allocations::without_allocations(|| {
                assert!(matches!(
                    service.dispatch(&mut kernel),
                    Ok(Dispatch::Completed { .. })
                ));
                assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Idle));
                kernel.step().unwrap();
            });
            assert_eq!(polls.get(), 2);
            service.revoke().unwrap();
        }
        assert_eq!(service.bound_owner().inspection().in_flight_operations, 0);
        assert!(revoked.get());
    }
}

fn paired_owners(bus: Bus, clock: Clock) -> (I2cHostCall<Bus>, MonotonicClockHostCall<Clock>) {
    let bus_contract = I2cContract::prepare().unwrap();
    let clock_contract = MonotonicClockContract::prepare().unwrap();
    let bus_scope = scope(bus_contract.kind(), I2C_CALL, I2C_MAXIMUM_BYTES, 1);
    let clock_scope = scope(
        clock_contract.kind(),
        CLOCK_CALL,
        CLOCK_MAXIMUM_BYTES,
        MAXIMUM_POLL_STEPS,
    );
    // Compose the fixture's draft placements, then seal their one final Plan
    // before either capability is issued or provider is bound.
    let (mut fragment, _, _, bus_placement) = fixture::selection_fixture::selected(
        &bus_scope,
        bus_contract.kind(),
        I2C_CALL,
        I2C_MAXIMUM_BYTES,
        "i2c",
        "i2c",
    );
    let (clock_fragment, _, _, clock_placement) = fixture::selection_fixture::selected(
        &clock_scope,
        clock_contract.kind(),
        CLOCK_CALL,
        CLOCK_MAXIMUM_BYTES,
        "clock",
        "clock",
    );
    fragment.placements.extend(clock_fragment.placements);
    fragment.startup_order.push(clock_placement.clone());
    fragment.expected_sign = vec![c::ExpectedSign::PlanFragmentReceived];
    for placement in [&bus_placement, &clock_placement] {
        fragment
            .expected_sign
            .push(c::ExpectedSign::PlacementPrepared(placement.clone()));
        fragment
            .expected_sign
            .push(c::ExpectedSign::PlacementTerminal(placement.clone()));
    }
    fragment.expected_sign.push(c::ExpectedSign::PlanTerminal);
    fragment.sign_storage_budget =
        c::mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
    let identity = c::PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = c::seal_plan(identity, vec![fragment]).fragments.remove(0);
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    let bus = bind_bus(
        bus_scope,
        bus,
        I2cCallSelection {
            contract: &bus_contract,
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement: &bus_placement,
        },
    );
    let clock = bind_clock(
        clock_scope,
        clock,
        ClockCallSelection {
            contract: &clock_contract,
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement: &clock_placement,
        },
    );
    assert_eq!((bus.node(), clock.node()), (NodeId(0), NodeId(1)));
    assert_eq!(
        bus.inspection().scope.plan_id,
        clock.inspection().scope.plan_id
    );
    (bus, clock)
}
type PairKernel = StoredExecutionKernel<Request, Signs, 2, 2, 1, 0, 0, 2>;
fn pair_kernel() -> PairKernel {
    let mut values = HostedValueStore::new(8, I2C_MAXIMUM_BYTES, 16384).unwrap();
    let bus = values.store(&bus_request(0x76)).unwrap();
    let clock = values.store(&clock_request()).unwrap();
    let mut routes = FixedRoutes::<0, 0>::new(2);
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<2>::new(1);
    for (node, maximum) in [(0, I2C_MAXIMUM_BYTES), (1, CLOCK_MAXIMUM_BYTES)] {
        bindings
            .install(
                NodeId(node),
                HostCallBinding {
                    call: HostCallId(0),
                    maximum_input_bytes: maximum,
                    maximum_output_bytes: maximum,
                },
            )
            .unwrap();
    }
    bindings.seal().unwrap();
    PairKernel::new_with_active_counts_and_host_calls(
        2,
        0,
        core::array::from_fn(|_| NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 8,
        }),
        core::array::from_fn(|i| {
            CordSpec::local(
                CordId(i as u16),
                (NodeId(i as u16), PortId(0)),
                (NodeId(i as u16), PortId(0)),
                CordCapacity {
                    slot_start: 0,
                    item_capacity: 1,
                    byte_capacity: I2C_MAXIMUM_BYTES,
                    pressure_policy: Default::default(),
                },
            )
        }),
        routes,
        bindings,
        [
            Request {
                input: bus,
                maximum: I2C_MAXIMUM_BYTES,
                tag: "completed",
                started: false,
            },
            Request {
                input: clock,
                maximum: CLOCK_MAXIMUM_BYTES,
                tag: "completed",
                started: false,
            },
        ],
        values,
        Signs {
            log: FixedSignLog::new(64 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
            fail: Rc::new(Cell::new(false)),
        },
    )
    .unwrap()
}
#[derive(Default)]
struct IoObserver {
    completed: usize,
}
impl conduit_mcu_host_runtime::native_runner::Observer<PairKernel> for IoObserver {
    type Error = ();
    fn before_step(&mut self, _: &mut PairKernel) -> Result<(), ()> {
        Ok(())
    }
    fn completed(&mut self, kernel: &mut PairKernel, dispatch: Dispatch) -> Result<(), ()> {
        let Dispatch::Completed { outcome, .. } = dispatch else {
            return Err(());
        };
        let bytes = kernel.host_value(outcome.output.unwrap().value).unwrap();
        assert_eq!(
            c::validate_canonical_structured_value(bytes)
                .unwrap()
                .variant_tag()
                .unwrap(),
            "completed"
        );
        self.completed += 1;
        Ok(())
    }
}
#[test]
fn shared_runner_services_both_owners_from_one_plan_and_retires_on_drain_or_cancel() {
    use conduit_mcu_host_runtime::native_runner::{Exit, Refusal as RunRefusal, Runner};
    for mode in 0..3 {
        let effects = Rc::new(Cell::new(0));
        let polls = Rc::new(Cell::new(0));
        let bus_revoked = Rc::new(Cell::new(false));
        let clock_revoked = Rc::new(Cell::new(false));
        let (mut bus, mut clock) = paired_owners(
            Bus {
                effects: effects.clone(),
                revoked: bus_revoked.clone(),
            },
            Clock {
                polls: polls.clone(),
                next: Rc::new(Cell::new((mode == 0).then_some(23))),
                revoked: clock_revoked.clone(),
            },
        );
        let mut kernel = pair_kernel();
        let mut output = [0; I2C_RESULT_VALUE_BYTES];
        let mut bus_service = Service::new(&mut bus, &mut output).unwrap();
        let mut clock_service = deadline::Service::new(&mut clock);
        let mut observer = IoObserver::default();
        let activity = NativeActivity::new();
        let cancelled = || mode == 1 && polls.get() > 0;
        let pause = Yield;
        let runner = Runner::new(&mut kernel, &mut observer, &pause, &cancelled);
        let runner = if mode == 2 {
            runner
        } else {
            runner.with_activity(&activity)
        };
        let result = allocations::without_allocations(|| {
            finish(runner.run_with_i2c_clock(&mut bus_service, &mut clock_service))
        });
        match mode {
            0 => {
                assert!(matches!(result, Ok(Exit::Drained)));
                assert_eq!(observer.completed, 2);
                assert_eq!(effects.get(), 1);
            }
            1 => {
                assert!(matches!(
                    result,
                    Err(RunRefusal::Kernel(SchedulerError::Cancelled))
                ));
                assert_eq!(polls.get(), 1);
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(RunRefusal::Synchronous(Refusal::MissingActivity))
                ));
                assert_eq!((effects.get(), polls.get()), (0, 0));
            }
        }
        assert_eq!(
            bus_revoked.get(),
            mode != 0,
            "normal drain must preserve idle bus custody"
        );
        assert!(clock_revoked.get());
        assert_eq!(
            bus_service.bound_owner().inspection().lifecycle,
            c::CapabilityLifecycle::Revoked
        );
        assert_eq!(
            bus_service.bound_owner().inspection().in_flight_operations,
            0
        );
        assert_eq!(
            clock_service
                .bound_owner()
                .inspection()
                .in_flight_operations,
            0
        );
    }
}

#[test]
fn deadline_storage_pressure_precedes_start_and_revoked_owner_returns_cancelled() {
    for pressure in [false, true] {
        let polls = Rc::new(Cell::new(0));
        let mut owner = clock(
            polls.clone(),
            Rc::new(Cell::new(Some(23))),
            Rc::new(Cell::new(false)),
        );
        let input = clock_request();
        let mut kernel = request_kernel(
            &input,
            CLOCK_MAXIMUM_BYTES,
            CLOCK_MAXIMUM_BYTES,
            if pressure { input.len() as u32 } else { 4096 },
            "completed",
            Rc::new(Cell::new(false)),
        );
        kernel.step().unwrap();
        let mut service = deadline::Service::new(&mut owner);
        if pressure {
            assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Pressure));
            assert_eq!(service.bound_owner().inspection().in_flight_operations, 0);
        } else {
            service.revoke().unwrap();
            let Dispatch::Completed { outcome, .. } = service.dispatch(&mut kernel).unwrap() else {
                panic!("missing cancellation")
            };
            assert_eq!(outcome.disposition, HostCallDisposition::Cancelled);
            assert_eq!(service.dispatch(&mut kernel), Ok(Dispatch::Idle));
        }
        assert_eq!(polls.get(), 0);
        service.revoke().unwrap();
        kernel.cancel().unwrap();
    }
}
#[test]
fn rejected_i2c_completion_never_repeats_the_physical_transaction() {
    let effects = Rc::new(Cell::new(0));
    let revoked = Rc::new(Cell::new(false));
    let fail = Rc::new(Cell::new(false));
    let mut owner = Interrupt {
        inner: bus(effects.clone(), revoked.clone()),
        fail: fail.clone(),
    };
    let mut kernel = request_kernel(
        &bus_request(0x76),
        I2C_MAXIMUM_BYTES,
        I2C_RESULT_VALUE_BYTES as u32,
        8192,
        "completed",
        fail.clone(),
    );
    kernel.step().unwrap();
    let mut output = [0; I2C_RESULT_VALUE_BYTES];
    let mut service = Service::new(&mut owner, &mut output).unwrap();
    let activity = NativeActivity::new();
    assert!(matches!(
        service.dispatch(&mut kernel, &activity),
        Err(Refusal::Kernel(SchedulerError::Sign(_)))
    ));
    assert_eq!(effects.get(), 1);
    fail.set(false);
    assert_eq!(service.dispatch(&mut kernel, &activity), Ok(Dispatch::Idle));
    assert_eq!(effects.get(), 1);
    service.revoke().unwrap();
    kernel.cancel().unwrap();
    assert!(revoked.get());
}
