//! Exercise the new realization through the actual fixed-capacity scheduler.
use super::*;
use conduit_kernel::{
    scheduler::{
        AssignedPressurePolicy, CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerError,
        SchedulerStatus,
    },
    CordEndpoint, CordId, FixedRoutes, FixedSignLog, NodeId, RouteRange, RouteTarget,
};

enum Back {
    Source {
        sent: bool,
        staged: bool,
    },
    Expression(StaticScalarExpressionBack),
    Sink {
        value: Option<u8>,
        staged: Option<u8>,
    },
}
impl StepBack<1> for Back {
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Expression(back) => back.step(io, input),
            Self::Source { sent, staged } => {
                *staged = false;
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send_prepared(PortId(0), 1).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Sink { value, staged } => {
                *staged = None;
                if value.is_some() {
                    return StepOutcome::Complete;
                }
                let Some(bytes) = input.input(PortId(0)) else {
                    return StepOutcome::Await;
                };
                assert_eq!(bytes.len(), 1);
                io.consume(PortId(0)).unwrap();
                *staged = Some(bytes[0]);
                StepOutcome::Progress
            }
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Expression(back) => {
                <StaticScalarExpressionBack as StepBack<1>>::prepared_output(back, port)
            }
            Self::Source { staged: true, .. } if port == PortId(0) => Some(&[41]),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Expression(back) => {
                <StaticScalarExpressionBack as StepBack<1>>::step_committed(back)
            }
            Self::Source { sent, staged } => {
                *sent |= *staged;
                *staged = false;
            }
            Self::Sink { value, staged } => {
                if let Some(next) = staged.take() {
                    *value = Some(next);
                }
            }
        }
    }
}
type Kernel = FixedScheduler<Back, FixedValueStore<4, 1>, FixedSignLog<128>, 3, 2, 1, 2, 3, 2>;
fn kernel(fuel: u16) -> Kernel {
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
    Kernel::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 4,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: fuel,
            },
            NodeSpec {
                input_cords: [Some(CordId(1))],
                maximum_step_fuel: 4,
            },
        ],
        core::array::from_fn(|index| {
            CordSpec::new(
                CordId(index as u16),
                local(index as u16),
                local(index as u16 + 1),
                CordCapacity {
                    slot_start: index as u16,
                    item_capacity: 1,
                    byte_capacity: 1,
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
            Back::Expression(resident(PortTemporal::Value)),
            Back::Sink {
                value: None,
                staged: None,
            },
        ],
        FixedValueStore::new(4).unwrap(),
        FixedSignLog::new((128 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32)
            .unwrap(),
    )
    .unwrap()
}
#[test]
fn resident_back_delivers_through_the_existing_kernel_without_allocating() {
    let (mut play, cost) = allocation_probe::observe(|| kernel(5));
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    let (status, cost) = allocation_probe::observe(|| {
        let mut status = SchedulerStatus::Idle;
        for _ in 0..16 {
            status = play.step().unwrap();
            if status == SchedulerStatus::Drained {
                break;
            }
        }
        status
    });
    assert_eq!(status, SchedulerStatus::Drained);
    assert!(matches!(
        play.drivers()[2],
        Back::Sink {
            value: Some(42),
            ..
        }
    ));
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
#[test]
fn insufficient_private_or_io_fuel_is_an_atomic_kernel_refusal() {
    for fuel in [2, 4] {
        let mut play = kernel(fuel);
        let mut refused = false;
        for _ in 0..16 {
            match play.step() {
                Err(SchedulerError::StepFuelExceeded) => {
                    refused = true;
                    break;
                }
                Ok(_) => (),
                Err(other) => panic!("wrong refusal: {other:?}"),
            }
        }
        assert!(refused);
        assert!(matches!(play.drivers()[2], Back::Sink { value: None, .. }));
    }
}
