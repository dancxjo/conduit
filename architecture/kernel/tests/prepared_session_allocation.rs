#![cfg(feature = "alloc")]
//! Actual local value queues, prepared outputs, resident pins and evidence handoff.
use conduit_kernel::scheduler::{prepared_session::*, *};
use conduit_kernel::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

enum Driver {
    Source {
        bytes: [u8; 8],
        sent: bool,
        staged: bool,
    },
    Sink {
        received: Option<u64>,
        staged: Option<u64>,
    },
}
impl StepBack<1> for Driver {
    fn step(&mut self, io: &mut StepIo<1>, input: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Source { sent: true, .. } => StepOutcome::Complete,
            Self::Source { staged, .. } => {
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send_prepared(PortId(0), 8).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Sink { staged, .. } => {
                let Some(bytes) = input.input(PortId(0)) else {
                    return StepOutcome::Await;
                };
                *staged = Some(u64::from_le_bytes(bytes.try_into().unwrap()));
                io.consume(PortId(0)).unwrap();
                StepOutcome::Complete
            }
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Source { bytes, .. } if port == PortId(0) => Some(bytes),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source { sent, staged, .. } if *staged => {
                *sent = true;
                *staged = false;
            }
            Self::Sink { received, staged } => {
                if let Some(value) = staged.take() {
                    *received = Some(value);
                }
            }
            _ => {}
        }
    }
}
impl ReusableStepBack<1> for Driver {
    type Next = Option<u64>;
    fn preflight_next(&self, next: &Self::Next, _: &dyn ValueStorage) -> bool {
        match self {
            Self::Source { sent, staged, .. } => *sent && !*staged && next.is_some(),
            Self::Sink { received, staged } => {
                received.is_some() && staged.is_none() && next.is_none()
            }
        }
    }
    fn begin_next(&mut self, next: Self::Next) {
        match self {
            Self::Source {
                bytes,
                sent,
                staged,
            } => {
                *bytes = next.unwrap().to_le_bytes();
                *sent = false;
                *staged = false;
            }
            Self::Sink { received, staged } => {
                *received = None;
                *staged = None;
            }
        }
    }
}
type Log = FixedSignLog<64>;
fn log() -> Log {
    Log::new(64 * core::mem::size_of::<KernelEvent>() as u32).unwrap()
}
#[test]
fn four_value_invocations_reuse_queues_and_store_without_boundary_or_play_allocation() {
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
    let mut values = FixedValueStore::<3, 8>::new(24).unwrap();
    let resident = values.store(&[9]).unwrap();
    let kernel = FixedScheduler::<_, _, _, 2, 1, 1, 1, 2, 1>::new(
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
                byte_capacity: 8,
                pressure_policy: AssignedPressurePolicy::PreserveOrder,
            },
        )],
        routes,
        [
            Driver::Source {
                bytes: 0u64.to_le_bytes(),
                sent: false,
                staged: false,
            },
            Driver::Sink {
                received: None,
                staged: None,
            },
        ],
        values,
        log(),
    )
    .unwrap();
    let mut session =
        PreparedKernelSession::new(Box::new(kernel)).unwrap_or_else(|_| panic!("fresh"));
    let address = session.kernel() as *const _ as usize;
    let mut fresh: Vec<_> = (0..4).map(|_| log()).collect();
    let mut previous = Vec::with_capacity(4);
    let ((), observed) = allocation_probe::observe(|| {
        for invocation in 0..4 {
            let mut drained = false;
            for _ in 0..8 {
                if session.step().unwrap() == SchedulerStatus::Drained {
                    drained = true;
                    break;
                }
            }
            assert!(drained);
            assert!(
                matches!(&session.kernel().drivers()[1],Driver::Sink { received:Some(value),.. } if *value==invocation)
            );
            let receipt = session
                .begin_next([Some(invocation + 1), None], fresh.pop().unwrap())
                .unwrap_or_else(|_| panic!("normal local value boundary"));
            previous.push(receipt);
            assert_eq!(session.kernel().values().get(resident).unwrap(), &[9]);
            assert_eq!(session.kernel().values().used_items(), 1);
            assert_eq!(session.kernel() as *const _ as usize, address);
        }
    });
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    let reference: Vec<_> = previous[0].signs.events().collect();
    for old in previous {
        assert_eq!(old.signs.events().collect::<Vec<_>>(), reference);
    }
}
