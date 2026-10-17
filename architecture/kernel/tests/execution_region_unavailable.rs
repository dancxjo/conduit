#![cfg(not(feature = "alloc"))]
//! Products without a reviewed region offer keep their single-Step path.
use conduit_kernel::scheduler::*;
use conduit_kernel::*;

#[derive(Clone, Copy)]
struct Close;
impl StepBack<1> for Close {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0)).unwrap();
        }
        StepOutcome::Complete
    }
}

#[test]
fn unavailable_region_refuses_and_each_original_step_still_drains() {
    type Play = FixedScheduler<Close, FixedValueStore<1, 1>, FixedSignLog<32>, 2, 1, 1, 1, 2, 1>;
    let mut routes = FixedRoutes::<2, 1>::new(1);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut play = Play::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 8,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 8,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 1,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        [Close; 2],
        FixedValueStore::new(1).unwrap(),
        FixedSignLog::new((32 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
    )
    .unwrap();
    assert_eq!(
        play.bind_execution_chain(&[NodeId(0), NodeId(1)]),
        Err(SchedulerError::InvalidPlan)
    );
    assert_eq!(
        play.step(),
        Ok(SchedulerStatus::Progress { node: NodeId(0) })
    );
    assert_eq!((play.decisions(), play.logical_invocations()), (1, 1));
    assert_eq!(play.step(), Ok(SchedulerStatus::Drained));
    assert_eq!((play.decisions(), play.logical_invocations()), (2, 2));
}
