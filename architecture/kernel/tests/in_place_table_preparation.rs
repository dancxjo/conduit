#![cfg(feature = "alloc")]
#[path = "common/preparation_allocation.rs"]
mod allocation;
#[allow(dead_code)]
#[path = "common/boxed_kernel_fixture.rs"]
mod fixture;
use conduit_kernel::{scheduler::*, *};
use fixture::*;
use std::rc::Rc;

fn write(
    nodes: &mut [NodeSpec<1>; 1],
    cords: &mut [CordSpec; 1],
    routes: &mut FixedRoutes<1, 0>,
    calls: &mut FixedHostCallBindings<0>,
) -> Result<(usize, usize), KernelPreparationError> {
    assert_eq!(nodes[0].maximum_step_fuel, 0);
    assert_eq!(cords[0], CordSpec::inactive());
    nodes[0].maximum_step_fuel = 1;
    routes.seal()?;
    calls.seal()?;
    Ok((1, 0))
}
#[test]
fn one_funded_allocation_is_preserved_through_bind_and_execution() {
    let drops = Rc::new(Drops::default());
    let (prepared, cost) = allocation::observe(false, || {
        prepare_written(&drops, size_of::<Kernel>(), write).unwrap()
    });
    assert_eq!(
        (cost.requests, cost.allocations, cost.deallocations),
        (1, 1, 0)
    );
    let address = prepared.storage_address();
    let (mut kernel, cost) = allocation::observe(false, || prepared.bind([Driver(drops.clone())]));
    assert_eq!((cost.requests, cost.deallocations), (0, 0));
    assert_eq!((&*kernel as *const Kernel).cast::<()>(), address);
    kernel.run(4).unwrap();
    assert_eq!(kernel.step().unwrap(), SchedulerStatus::Drained);
    drop(kernel);
    assert_eq!(
        (drops.values.get(), drops.signs.get(), drops.drivers.get()),
        (1, 1, 1)
    );
}
#[test]
fn writer_refusal_unsealed_topology_invalid_counts_and_unwind_drop_once() {
    for mode in 0..4 {
        let drops = Rc::new(Drops::default());
        let owner = Some(Driver(drops.clone()));
        let (result, cost) = allocation::observe(false, || {
            prepare_written(&drops, usize::MAX, |n, c, r, h| {
                if mode == 0 {
                    return Err(SchedulerError::InvalidPlan.into());
                }
                if mode == 1 {
                    return Ok((1, 0));
                }
                write(n, c, r, h)?;
                if mode == 2 {
                    Ok((2, 0))
                } else {
                    n[0].maximum_step_fuel = 0;
                    Ok((1, 0))
                }
            })
        });
        assert!(result.is_err());
        assert_eq!(
            (cost.requests, cost.allocations, cost.deallocations),
            (1, 1, 1)
        );
        assert_eq!(
            (drops.values.get(), drops.signs.get(), drops.drivers.get()),
            (1, 1, 0)
        );
        assert!(owner.is_some());
    }
    let drops = Rc::new(Drops::default());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = prepare_written(&drops, usize::MAX, |_, _, _, _| panic!("writer cancelled"));
    }));
    assert!(result.is_err());
    assert_eq!((drops.values.get(), drops.signs.get()), (1, 1));
}
#[test]
fn short_funding_and_allocator_refusal_do_not_invoke_writer() {
    for fail in [false, true] {
        let drops = Rc::new(Drops::default());
        let (result, cost) = allocation::observe(fail, || {
            prepare_written(&drops, if fail { usize::MAX } else { 0 }, |_, _, _, _| {
                panic!("unfunded writer")
            })
        });
        assert!(result.is_err());
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.requests, usize::from(fail));
        assert_eq!((drops.values.get(), drops.signs.get()), (1, 1));
    }
}

#[test]
fn terminal_gate_checks_inactive_actual_and_expected_rows_in_guarded_storage() {
    struct Back;
    impl StepBack<1> for Back {
        fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
            StepOutcome::Await
        }
    }
    type K = FixedScheduler<Back, FixedValueStore<1, 1>, FixedSignLog<16>, 2, 1, 1, 1, 1, 0, 0, 1>;
    let law = AssignedTerminalTransduction {
        input: PortId(0),
        output: PortId(0),
        normal_close: AssignedNormalCloseTransduction::Consume,
        abnormal: AssignedAbnormalTransduction::NotAccepted,
        cancellation: AssignedCancellationTransduction::NotCancellable,
    };
    for (expected, actual) in [
        ([[None], [None]], [[None], [Some(law)]]),
        ([[None], [Some(law)]], [[None], [Some(law)]]),
    ] {
        let (result, cost) = allocation::observe(false, || {
            K::prepare_boxed_with_table_writer(
                FixedValueStore::new(1).unwrap(),
                FixedSignLog::new((16 * size_of::<KernelEvent>()) as u32).unwrap(),
                usize::MAX,
                &expected,
                &actual,
                core::convert::identity,
                |nodes, _, routes, calls| {
                    nodes[0].maximum_step_fuel = 1;
                    routes.seal()?;
                    calls.seal()?;
                    Ok((1, 0))
                },
            )
        });
        assert_eq!(
            result.err(),
            Some(KernelPreparationError::Topology(
                SchedulerError::InvalidPlan
            ))
        );
        assert_eq!(
            (cost.requests, cost.allocations, cost.deallocations),
            (1, 1, 1)
        );
    }
}
