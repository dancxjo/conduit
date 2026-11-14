//! One sealed Plan and shared runner with two original native owners.
use super::*;
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
