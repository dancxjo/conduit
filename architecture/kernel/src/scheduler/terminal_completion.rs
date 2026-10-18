//! Completion-prefix cost after all but the final node have terminated.
use super::*;

#[derive(Clone, Copy)]
struct Tail {
    calls: u32,
    complete: bool,
}
impl StepBack<1> for Tail {
    fn step(&mut self, io: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        self.calls += 1;
        if self.complete {
            StepOutcome::Complete
        } else if self.calls <= 9 {
            io.consume_fuel(io.remaining_fuel()).unwrap();
            StepOutcome::Yield
        } else {
            StepOutcome::Await
        }
    }
}

fn specimen<const N: usize>(trial: usize) {
    let mut routes = FixedRoutes::<N, 0>::new(1);
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, N, 0, 1, 1, N, 0, 0, 1>::new_with_active_counts(
        N,
        0,
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 8,
        }; N],
        [],
        routes,
        core::array::from_fn(|node| Tail {
            calls: 0,
            complete: node + 1 != N,
        }),
        ObservedValues::<4>::new(),
        FixedSignLog::<2048>::new(2048 * core::mem::size_of::<crate::KernelEvent>() as u32)
            .unwrap(),
    )
    .unwrap();
    for node in 0..N {
        assert_eq!(
            scheduler.step(),
            Ok(SchedulerStatus::Progress {
                node: NodeId(node as u16)
            })
        );
    }
    scheduler.scan_counts.set(Default::default());
    let allocation = AllocationSample::begin();
    let started = Instant::now();
    for _ in 0..9 {
        assert_eq!(
            scheduler.step(),
            Ok(SchedulerStatus::Progress {
                node: NodeId((N - 1) as u16)
            })
        );
    }
    for _ in 0..8 {
        assert_eq!(scheduler.step(), Ok(SchedulerStatus::Idle));
    }
    let elapsed_ns = started.elapsed().as_nanos();
    let allocations = allocation.finish();
    assert_eq!(allocations.allocations + allocations.reallocations, 0);
    assert_eq!(scheduler.drivers[N - 1].calls, 10);
    assert!(!scheduler.completed[N - 1]);
    assert!(!scheduler.has_ready_work());
    let counts = scheduler.scan_counts.get();
    assert_eq!(
        counts.terminal_node_probes,
        if N >= 2 * core::mem::size_of::<usize>() {
            0
        } else {
            17 * N
        }
    );
    assert_eq!(scheduler.completed_count(), N - 1);
    assert_eq!(
        scheduler.completed_count(),
        scheduler.completed.iter().filter(|done| **done).count()
    );
    let events = std::format!(
        "{:?}",
        scheduler.signs.events().collect::<std::vec::Vec<_>>()
    );
    std::println!("TERMINAL_COMPLETION_ROW {{\"nodes\":{N},\"trial\":{trial},\"elapsed_ns\":{elapsed_ns},\"terminal_node_probes\":{},\"allocations\":{},\"reallocations\":{},\"kernel_bytes\":{},\"events\":{events:?}}}", counts.terminal_node_probes, allocations.allocations, allocations.reallocations, core::mem::size_of_val(&scheduler));
    scheduler.cancel().unwrap();
    assert_eq!(scheduler.step(), Ok(SchedulerStatus::Cancelled));
    assert!(scheduler.try_retire().is_ok());
}

#[test]
fn completion_prefix_is_observed_after_progress_and_idle() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            specimen::<1>(0);
            specimen::<8>(0);
            specimen::<32>(0);
            specimen::<256>(0);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[ignore = "manual optimized completion-prefix measurements"]
fn capture_completion_prefix() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            for trial in 0..8 {
                specimen::<1>(trial);
                specimen::<8>(trial);
                specimen::<32>(trial);
                specimen::<256>(trial);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn completion_count_uses_active_prefix_and_does_not_double_retire() {
    let mut routes = FixedRoutes::<8, 0>::new(1);
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 8, 0, 1, 1, 8, 0, 0, 1>::new_with_active_counts(
        3,
        0,
        [NodeSpec {
            input_cords: [None],
            maximum_step_fuel: 8,
        }; 8],
        [],
        routes,
        [Tail {
            calls: 0,
            complete: true,
        }; 8],
        ObservedValues::<4>::new(),
        FixedSignLog::<64>::new(64 * core::mem::size_of::<crate::KernelEvent>() as u32).unwrap(),
    )
    .unwrap();
    for node in 0..3 {
        assert_eq!(scheduler.completed_count(), node);
        assert_eq!(
            scheduler.step().unwrap(),
            if node == 2 {
                SchedulerStatus::Drained
            } else {
                SchedulerStatus::Progress {
                    node: NodeId(node as u16),
                }
            }
        );
        assert_eq!(scheduler.completed_count(), node + 1);
    }
    scheduler.mark_completed(2);
    assert_eq!(scheduler.completed_count(), 3);
    for _ in 0..4 {
        assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Drained);
    }
    assert!(!scheduler.completed[3..].iter().any(|done| *done));
    assert!(scheduler.try_retire().is_ok());
}

#[cfg(feature = "alloc")]
#[test]
fn boxed_preparation_initializes_completion_count() {
    let mut routes = FixedRoutes::<1, 0>::new(1);
    routes.seal().unwrap();
    let mut bindings = FixedHostCallBindings::<0>::new(1);
    bindings.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 1, 0, 1, 1, 1, 0, 0, 1>::new_boxed_with_active_counts_and_host_calls(
        1, 0, [NodeSpec { input_cords: [None], maximum_step_fuel: 8 }], [], routes, bindings,
        [Tail { calls: 0, complete: true }], ObservedValues::<4>::new(),
        FixedSignLog::<16>::new(16 * core::mem::size_of::<crate::KernelEvent>() as u32).unwrap(),
    ).unwrap();
    assert_eq!(scheduler.completed_count(), 0);
    assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Drained);
    assert_eq!(scheduler.completed_count(), 1);
    assert_eq!(scheduler.step().unwrap(), SchedulerStatus::Drained);
}
