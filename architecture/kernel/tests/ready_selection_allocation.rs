//! Actual fixed scheduler fairness and allocation guard across packed-byte boundaries.
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, NodeSpec, SchedulerStatus, StepBack, StepInputBytes, StepIo,
    StepOutcome,
};
use conduit_kernel::{FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[derive(Clone, Copy)]
struct Yielding(u8);
impl StepBack<1> for Yielding {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        if self.0 == 0 {
            StepOutcome::Complete
        } else {
            io.consume_fuel(io.remaining_fuel()).unwrap();
            self.0 -= 1;
            StepOutcome::Yield
        }
    }
    fn cancel(&mut self) {}
}
#[test]
fn thirty_two_ready_nodes_remain_fair_without_play_allocation() {
    let mut routes = FixedRoutes::<1, 1>::new(1);
    routes.seal().unwrap();
    let mut play = FixedScheduler::<_, _, _, 32, 1, 1, 1, 1, 1>::new_with_active_counts(
        32,
        0,
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 8,
        }; 32],
        [CordSpec::inactive()],
        routes,
        [Yielding(4); 32],
        FixedValueStore::<1, 1>::new(1).unwrap(),
        FixedSignLog::<512>::new((512 * core::mem::size_of::<KernelEvent>()) as u32).unwrap(),
    )
    .unwrap();
    let (_, observed) = allocation_probe::observe(|| {
        for round in 0..5 {
            for node in 0..32 {
                assert_eq!(
                    play.step().unwrap(),
                    if round == 4 && node == 31 {
                        SchedulerStatus::Drained
                    } else {
                        SchedulerStatus::Progress { node: NodeId(node) }
                    }
                );
            }
        }
    });
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
}
