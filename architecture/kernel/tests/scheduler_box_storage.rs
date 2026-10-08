#![cfg(feature = "alloc")]
use conduit_kernel::{scheduler::*, *};
#[path = "../../../semantics/ai/src/numeric_allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[derive(Clone, Copy)]
struct Done;
impl StepBack<1> for Done {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Complete
    }
    fn cancel(&mut self) {}
}
type Run = FixedScheduler<Done, FixedValueStore<1, 1>, FixedSignLog<16>, 1, 0, 1, 1, 1, 1, 1, 1>;
fn prepare(
    active: usize,
    limits: SchedulerBoxStorageLimits,
) -> Result<(Box<Run>, SchedulerBoxStorageReceipt), SchedulerBoxPreparationRefusal> {
    let mut routes = FixedRoutes::new(1);
    routes.seal().unwrap();
    let mut hosts = FixedHostCallBindings::new(1);
    hosts.seal().unwrap();
    Run::new_boxed_with_storage_limits(
        active,
        0,
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 1,
        }],
        [],
        routes,
        hosts,
        [Done],
        FixedValueStore::new(1).unwrap(),
        FixedSignLog::new(16 * std::mem::size_of::<KernelEvent>() as u32).unwrap(),
        limits,
    )
}
#[test]
fn complete_fixed_root_is_reserved_before_boxing_and_guards_are_retained() {
    let (r, o) = allocation_probe::observe(Run::boxed_storage_reservation);
    assert_eq!((o.allocations, o.reallocations), (0, 0));
    assert_eq!(r.retained_root_bytes, std::mem::size_of::<Run>());
    for which in 0..2 {
        let mut limits = SchedulerBoxStorageLimits {
            maximum_preparation_requested_bytes: r.preparation_requested_bytes_bound,
            maximum_retained_root_bytes: r.retained_root_bytes,
        };
        if which == 0 {
            limits.maximum_preparation_requested_bytes -= 1;
        } else {
            limits.maximum_retained_root_bytes -= 1;
        }
        let (result, o) = allocation_probe::observe(|| prepare(1, limits));
        assert!(matches!(
            result,
            Err(SchedulerBoxPreparationRefusal::Capacity)
        ));
        assert_eq!((o.allocations, o.reallocations), (0, 0));
    }
    let limits = SchedulerBoxStorageLimits {
        maximum_preparation_requested_bytes: r.preparation_requested_bytes_bound,
        maximum_retained_root_bytes: r.retained_root_bytes,
    };
    let (result, o) = allocation_probe::observe(|| prepare(0, limits));
    assert!(matches!(
        result,
        Err(SchedulerBoxPreparationRefusal::Scheduler(
            SchedulerError::InvalidActiveCapacity
        ))
    ));
    assert_eq!((o.allocations, o.reallocations), (0, 0));
    let (result, o) = allocation_probe::observe(|| prepare(1, limits));
    let (mut run, actual) = result.unwrap();
    assert_eq!(actual, r);
    assert_eq!(o.requested_bytes, r.preparation_requested_bytes_bound);
    assert_eq!(o.live_bytes, r.retained_root_bytes);
    let mut drained = false;
    for _ in 0..4 {
        let (status, o) = allocation_probe::observe(|| run.step());
        assert_eq!((o.allocations, o.reallocations), (0, 0));
        if status.unwrap() == SchedulerStatus::Drained {
            drained = true;
            break;
        }
    }
    assert!(drained);
    println!(
        "complete scheduler root requested={} retained={}",
        r.preparation_requested_bytes_bound, r.retained_root_bytes
    );
}
