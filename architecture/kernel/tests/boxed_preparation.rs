#![cfg(feature = "alloc")]
#[path = "common/preparation_allocation.rs"]
mod allocation;
#[path = "common/boxed_kernel_fixture.rs"]
mod fixture;
use conduit_kernel::scheduler::{KernelPreparationError as Error, SchedulerError, SchedulerStatus};
use fixture::*;
use std::rc::Rc;

#[test]
fn invalid_topology_and_short_ceiling_refuse_before_requesting_storage_or_backs() {
    let drops = Rc::new(Drops::default());
    let driver = Some(Driver(drops.clone()));
    for (active, ceiling, expected) in [
        (
            0,
            usize::MAX,
            Error::Topology(SchedulerError::InvalidActiveCapacity),
        ),
        (
            1,
            size_of::<Kernel>() - 1,
            Error::Capacity {
                required: size_of::<Kernel>(),
                available: size_of::<Kernel>() - 1,
            },
        ),
    ] {
        let (result, cost) = allocation::observe(false, || prepare(&drops, ceiling, active));
        assert_eq!(result.err(), Some(expected));
        assert_eq!(cost.requests, 0);
        assert_eq!(drops.drivers.get(), 0);
    }
    assert!(driver.is_some());
    assert_eq!((drops.values.get(), drops.signs.get()), (2, 2));
}

#[test]
fn allocation_failure_is_a_refusal_and_preserves_the_untransferred_driver() {
    let drops = Rc::new(Drops::default());
    let driver = Some(Driver(drops.clone()));
    let (result, cost) = allocation::observe(true, || prepare(&drops, usize::MAX, 1));
    assert_eq!(result.err(), Some(Error::Allocation));
    assert_eq!(
        (cost.requests, cost.allocations, cost.deallocations),
        (1, 0, 0)
    );
    assert_eq!(
        (drops.values.get(), drops.signs.get(), drops.drivers.get()),
        (1, 1, 0)
    );
    assert!(driver.is_some());
}

#[test]
fn late_workset_failure_releases_earlier_storage_and_every_initialized_store_once() {
    let drops = Rc::new(Drops::default());
    let drivers = [Some(Driver(drops.clone())), Some(Driver(drops.clone()))];
    let (first, cost) = allocation::observe(false, || prepare(&drops, usize::MAX, 1).unwrap());
    assert_eq!((cost.requests, cost.allocations), (1, 1));
    assert_eq!(first.storage_bytes(), size_of::<Kernel>());
    assert_eq!(size_of_val(&first), 3 * size_of::<usize>());
    let remaining = 2 * size_of::<Kernel>() - 1 - first.storage_bytes();
    let (second, cost) = allocation::observe(false, || prepare(&drops, remaining, 1));
    assert!(matches!(second, Err(Error::Capacity { .. })));
    assert_eq!(cost.requests, 0);
    let (second, cost) = allocation::observe(true, || prepare(&drops, usize::MAX, 1));
    assert_eq!(second.err(), Some(Error::Allocation));
    assert_eq!(cost.allocations, 0);
    let ((), cost) = allocation::observe(false, || drop(first));
    assert_eq!(cost.deallocations, 1);
    assert_eq!(
        (drops.values.get(), drops.signs.get(), drops.drivers.get()),
        (3, 3, 0)
    );
    assert!(drivers.iter().all(Option::is_some));
}

#[test]
fn binding_keeps_the_allocation_and_kernel_execution_retirement_releases_original_owners() {
    let drops = Rc::new(Drops::default());
    let mut driver = Some(Driver(drops.clone()));
    let prepared = prepare(&drops, usize::MAX, 1).unwrap();
    let address = prepared.storage_address();
    let (mut kernel, cost) = allocation::observe(false, || prepared.bind([driver.take().unwrap()]));
    assert_eq!((cost.requests, cost.deallocations), (0, 0));
    assert_eq!((&*kernel as *const Kernel).cast::<()>(), address);
    assert_eq!(
        (drops.values.get(), drops.signs.get(), drops.drivers.get()),
        (0, 0, 0)
    );
    let (retired, cost) = allocation::observe(false, || {
        kernel.run(4).unwrap();
        assert_eq!(kernel.step().unwrap(), SchedulerStatus::Drained);
        (*kernel)
            .try_retire()
            .unwrap_or_else(|_| panic!("drained kernel refused retirement"))
    });
    assert_eq!((cost.requests, cost.deallocations), (0, 1));
    assert_eq!(drops.drivers.get(), 0);
    drop(retired);
    assert_eq!(
        (drops.values.get(), drops.signs.get(), drops.drivers.get()),
        (1, 1, 1)
    );
}
