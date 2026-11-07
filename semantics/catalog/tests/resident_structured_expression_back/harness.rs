use conduit_core::PortTemporal;
use conduit_kernel::{
    scheduler::{
        prepared_session::ReusableStepBack, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    FixedValueStore, PortId, ValueRef,
};
fn frame(bytes: &[u8], ready: bool, fuel: u16) -> StepIo<1> {
    StepIo::test_frame(
        [Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: bytes.len() as u32,
        })],
        [false],
        [ready.then_some(2048)],
        None,
        fuel,
    )
}
fn lifecycle() {
    image::refuse_bounds();
    image::refuse_excess_work_and_mismatched_child_guards();
    let mut back = image::back(PortTemporal::Value);
    let required = back.required_step_fuel();
    assert!(back.computation_fuel() > 1);
    assert_eq!(required, back.computation_fuel() + 2);
    let input = StepInputBytes::test_frame([Some(INPUT)], None);
    let mut blocked = frame(INPUT, false, required);
    assert_eq!(back.step(&mut blocked, &input), StepOutcome::Await);
    assert_eq!(blocked.remaining_fuel(), required);
    let mut short = frame(INPUT, true, required - 1);
    assert_eq!(back.step(&mut short, &input), StepOutcome::Yield);
    assert!(!short.test_consumed(PortId(0)));
    assert!(short.test_prepared_output().is_none());
    assert!(back.last_evaluation_refusal().is_none());
    for _ in 0..2 {
        let mut io = frame(INPUT, true, required);
        assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
        assert_eq!(io.remaining_fuel(), 0);
        assert_eq!(
            <image::Back as StepBack<1>>::prepared_output(&back, PortId(0)),
            Some(EXPECTED)
        );
    }
    <image::Back as StepBack<1>>::step_committed(&mut back);
    let values = FixedValueStore::<1, 2048>::new(1).unwrap();
    assert!(<image::Back as ReusableStepBack<1>>::preflight_next(
        &back,
        &(),
        &values
    ));
    <image::Back as ReusableStepBack<1>>::begin_next(&mut back, ());
    let mut io = frame(BAD, true, required);
    assert!(matches!(
        back.step(&mut io, &StepInputBytes::test_frame([Some(BAD)], None)),
        StepOutcome::Fail(_)
    ));
    assert!(!io.test_consumed(PortId(0)));
    assert!(io.test_prepared_output().is_none());
    assert!(matches!(
        back.last_evaluation_refusal(),
        Some(conduit_plot::StaticStructuredRefusal::Input(_))
    ));
    <image::Back as StepBack<1>>::cancel(&mut back);
    assert!(!<image::Back as ReusableStepBack<1>>::preflight_next(
        &back,
        &(),
        &values
    ));
    let mut flow = image::back(PortTemporal::Flow { closes: true });
    let mut io = frame(INPUT, true, required);
    assert_eq!(flow.step(&mut io, &input), StepOutcome::Progress);
    <image::Back as StepBack<1>>::step_committed(&mut flow);
    let mut closed = StepIo::test_frame([None], [true], [Some(2048)], None, 1);
    assert_eq!(
        flow.step(&mut closed, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(closed.test_consumed_closed(PortId(0)));
    // Closing intent cannot release the backing until its transaction commits.
    assert!(!flow.reset_after_retirement());
    <image::Back as StepBack<1>>::step_committed(&mut flow);
    assert!(flow.reset_after_retirement());
    assert!(<image::Back as StepBack<1>>::prepared_output(&flow, PortId(0)).is_none());
    let mut next = frame(INPUT, true, required);
    assert_eq!(flow.step(&mut next, &input), StepOutcome::Progress);
}
enum Back {
    Source { sent: bool, staged: bool },
    Expression(image::Back),
    Sink { complete: bool, staged: bool },
}
impl StepBack<1> for Back {
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Expression(b) => b.step(io, input),
            Self::Source { sent, staged } => {
                *staged = false;
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send_prepared(PortId(0), INPUT.len() as u32).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Sink { complete, staged } => {
                *staged = false;
                if *complete {
                    return StepOutcome::Complete;
                }
                let Some(bytes) = input.input(PortId(0)) else {
                    return StepOutcome::Await;
                };
                assert_eq!(bytes, EXPECTED);
                io.consume(PortId(0)).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Expression(b) => <image::Back as StepBack<1>>::prepared_output(b, port),
            Self::Source { staged: true, .. } if port == PortId(0) => Some(INPUT),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Expression(b) => <image::Back as StepBack<1>>::step_committed(b),
            Self::Source { sent, staged } => {
                *sent |= *staged;
                *staged = false;
            }
            Self::Sink { complete, staged } => {
                *complete |= *staged;
                *staged = false;
            }
        }
    }
}
fn kernel() {
    use conduit_kernel::{
        scheduler::{
            AssignedPressurePolicy, CordCapacity, CordSpec, FixedScheduler, NodeSpec,
            SchedulerStatus,
        },
        CordEndpoint, CordId, FixedRoutes, FixedSignLog, NodeId, RouteRange, RouteTarget,
    };
    let local = |node| CordEndpoint::Local {
        node: NodeId(node),
        port: PortId(0),
    };
    let mut routes = FixedRoutes::new(1);
    for node in 0..2 {
        routes
            .install(
                NodeId(node),
                PortId(0),
                RouteRange {
                    start: node,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(node),
                    sink: local(node + 1),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let expression = image::back(PortTemporal::Value);
    let fuel = expression.required_step_fuel();
    let mut k: FixedScheduler<Back, FixedValueStore<4, 2048>, FixedSignLog<128>, 3, 2, 1, 2, 3, 2> =
        FixedScheduler::new(
            [
                NodeSpec {
                    input_cords: [None],
                    maximum_step_fuel: 2,
                },
                NodeSpec {
                    input_cords: [Some(CordId(0))],
                    maximum_step_fuel: fuel,
                },
                NodeSpec {
                    input_cords: [Some(CordId(1))],
                    maximum_step_fuel: 2,
                },
            ],
            core::array::from_fn(|i| {
                CordSpec::new(
                    CordId(i as u16),
                    local(i as u16),
                    local(i as u16 + 1),
                    CordCapacity {
                        slot_start: i as u16,
                        item_capacity: 1,
                        byte_capacity: 2048,
                        pressure_policy: AssignedPressurePolicy::PreserveOrder,
                    },
                )
            }),
            routes,
            [
                Back::Source {
                    sent: false,
                    staged: false,
                },
                Back::Expression(expression),
                Back::Sink {
                    complete: false,
                    staged: false,
                },
            ],
            FixedValueStore::new(4 * 2048).unwrap(),
            FixedSignLog::new((128 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32)
                .unwrap(),
        )
        .unwrap();
    let mut status = SchedulerStatus::Idle;
    for _ in 0..16 {
        status = k.step().unwrap();
        if status == SchedulerStatus::Drained {
            break;
        }
    }
    assert_eq!(status, SchedulerStatus::Drained);
    assert!(matches!(&k.drivers()[2], Back::Sink { complete: true, .. }));
}
fn main() {
    let ((), cost) = probe::observe(|| {
        lifecycle();
        kernel();
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
