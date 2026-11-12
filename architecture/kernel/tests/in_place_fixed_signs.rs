#![cfg(feature = "alloc")]
#[path = "common/preparation_allocation.rs"]
mod allocation;
#[allow(dead_code)]
#[path = "common/boxed_kernel_fixture.rs"]
mod fixture;
use conduit_kernel::{scheduler::*, *};
use fixture::{owned_values, Driver, Drops, Values};
use std::rc::Rc;
type Kernel = FixedScheduler<Driver, Values, FixedSignLog<64>, 1, 1, 1, 1, 1, 0, 0, 1>;
const SIGN_BYTES: u32 = (64 * size_of::<KernelEvent>()) as u32;
fn prepare(
    drops: &Rc<Drops>,
    budget: (u32, u16, u32),
    ceiling: usize,
    mode: u8,
) -> Result<PreparedKernelStorage<Driver, Kernel, 1>, KernelPreparationError> {
    Kernel::prepare_boxed_with_fixed_signs_and_table_writer(
        owned_values(drops),
        budget.0,
        budget.1,
        budget.2,
        ceiling,
        &[[None]],
        &[[None]],
        core::convert::identity,
        |nodes, _, routes, calls| {
            if mode == 1 {
                return Err(SchedulerError::InvalidPlan.into());
            }
            if mode == 2 {
                panic!("cancelled writer");
            }
            if mode == 3 {
                return Ok((1, 0));
            }
            assert_ne!(mode, 4, "unfunded callback");
            nodes[0].maximum_step_fuel = 1;
            routes.seal()?;
            calls.seal()?;
            Ok((1, 0))
        },
    )
}
#[test]
fn invalid_sign_budgets_refuse_before_allocation_or_callback_and_drop_values_once() {
    let remote = remote_sign_storage_bytes(12).unwrap();
    for budget in [
        (0, 12, remote),
        (SIGN_BYTES + 1, 12, remote),
        (SIGN_BYTES, 65, remote),
        (SIGN_BYTES, 0, 1),
        (SIGN_BYTES, 12, 0),
        (SIGN_BYTES, 12, remote + 1),
    ] {
        let drops = Rc::new(Drops::default());
        let (result, cost) = allocation::observe(false, || prepare(&drops, budget, usize::MAX, 4));
        assert_eq!(
            result.err(),
            Some(KernelPreparationError::Topology(SchedulerError::Sign(
                SignError::InvalidBudget
            )))
        );
        assert_eq!(
            (cost.requests, cost.allocations, cost.deallocations),
            (0, 0, 0)
        );
        assert_eq!(drops.values.get(), 1);
    }
}
#[test]
fn one_allocation_binds_without_allocation_and_retains_exact_sign_budgets_and_events() {
    let drops = Rc::new(Drops::default());
    let budget = (SIGN_BYTES, 12, remote_sign_storage_bytes(12).unwrap());
    let (prepared, cost) = allocation::observe(false, || {
        prepare(&drops, budget, size_of::<Kernel>(), 0).unwrap()
    });
    assert_eq!(
        (cost.requests, cost.allocations, cost.deallocations),
        (1, 1, 0)
    );
    let address = prepared.storage_address();
    let (mut kernel, cost) = allocation::observe(false, || prepared.bind([Driver(drops.clone())]));
    assert_eq!((cost.requests, cost.deallocations), (0, 0));
    assert_eq!((&*kernel as *const Kernel).cast::<()>(), address);
    let signs = kernel.signs();
    assert_eq!(
        (
            signs.item_capacity(),
            signs.byte_capacity(),
            signs.len(),
            signs.used_bytes()
        ),
        (64, SIGN_BYTES, 0, 0)
    );
    assert_eq!(
        (
            signs.remote_item_capacity(),
            signs.remote_byte_capacity(),
            signs.remote_len(),
            signs.remote_used_bytes()
        ),
        (12, budget.2, 0, 0)
    );
    kernel.run(4).unwrap();
    assert_eq!(kernel.step().unwrap(), SchedulerStatus::Drained);
    assert!(kernel
        .signs()
        .events()
        .any(|event| event.kind == KernelEventKind::BackCompleted));
    let (_, cost) = allocation::observe(false, || drop(kernel));
    assert_eq!(cost.deallocations, 1);
    assert_eq!((drops.values.get(), drops.drivers.get()), (1, 1));
}
#[test]
fn allocation_and_writer_failures_release_exactly_once_without_provider_transfer() {
    let budget = (SIGN_BYTES, 12, remote_sign_storage_bytes(12).unwrap());
    for (fail, ceiling, mode, allocated) in [
        (true, usize::MAX, 4, 0),
        (false, 0, 4, 0),
        (false, usize::MAX, 1, 1),
        (false, usize::MAX, 3, 1),
    ] {
        let drops = Rc::new(Drops::default());
        let original = Some(Driver(drops.clone()));
        let (result, cost) = allocation::observe(fail, || prepare(&drops, budget, ceiling, mode));
        assert!(result.is_err());
        assert_eq!(
            (cost.allocations, cost.deallocations),
            (allocated, allocated)
        );
        assert_eq!((drops.values.get(), drops.drivers.get()), (1, 0));
        assert!(original.is_some());
    }
    let drops = Rc::new(Drops::default());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = prepare(&drops, budget, usize::MAX, 2);
    }));
    assert!(result.is_err());
    assert_eq!(drops.values.get(), 1);
}
