//! Actual Root storage composition, held across a pending asynchronous poll.
use super::*;
use conduit_kernel::scheduler::KernelPreparationError;

pub fn run(
    receiver: &Receiver,
    bytes: &[u8],
    assignment: &crate::support::Assignment,
    library: &mut crate::library::Initialized,
) {
    use crate::root_preparer::{prepare_workset_storage as prepare, ValidationRefusal as E};
    let original = library.offers().map(|o| o.unwrap().as_ptr());
    let (result, cost) = crate::probe::observe(|| prepare(
        library, Some(&policy()), receiver.context(),
        receiver.command_with_contract(bytes, HOST_PREPARED_WORKSET_CONTRACT),
        assignment.maxima, 16384,
    ));
    let prepared = result.unwrap();
    let bytes_used = prepared.storage_bytes();
    assert_eq!(cost.allocations, 2);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(cost.requested_bytes, bytes_used);
    assert!(bytes_used <= 16384);
    let ((), released) = crate::probe::observe(|| drop(prepared));
    assert_eq!(released.deallocations, 2);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);

    for ceiling in [0, 8192, bytes_used - 1] {
        let (result, cost) = crate::probe::observe(|| prepare(
            library, Some(&policy()), receiver.context(),
            receiver.command_with_contract(bytes, HOST_PREPARED_WORKSET_CONTRACT),
            assignment.maxima, ceiling,
        ));
        assert!(matches!(result, Err(E::Storage(KernelPreparationError::Capacity { .. }))));
        drop(result);
        assert_eq!(cost.allocations, usize::from(ceiling != 0));
        assert_eq!(cost.allocations, cost.deallocations);
        assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    }
    for nth in 1..=2 {
        let (result, cost) = crate::probe::observe_refusing(nth, || prepare(
            library, Some(&policy()), receiver.context(),
            receiver.command_with_contract(bytes, HOST_PREPARED_WORKSET_CONTRACT),
            assignment.maxima, 16384,
        ));
        assert!(matches!(result, Err(E::Storage(KernelPreparationError::Allocation))));
        drop(result);
        assert_eq!(cost.refused_allocations, 1);
        assert_eq!(cost.allocations, cost.deallocations);
        assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    }
    let future = async {
        let prepared = prepare(
            library, Some(&policy()), receiver.context(),
            receiver.command_with_contract(bytes, HOST_PREPARED_WORKSET_CONTRACT),
            assignment.maxima, 16384,
        ).unwrap();
        assert_eq!(prepared.storage_bytes(), bytes_used);
        core::future::pending::<()>().await;
        drop(prepared);
    };
    let future_bytes = core::mem::size_of_val(&future);
    let mut future = Box::pin(future);
    let ((), cost) = crate::probe::observe(|| {
        use core::future::Future;
        let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
        assert!(future.as_mut().poll(&mut cx).is_pending());
    });
    assert_eq!((cost.allocations, cost.deallocations), (2, 0));
    let ((), cleanup) = crate::probe::observe(|| drop(future));
    assert_eq!((cleanup.allocations, cleanup.reallocations), (0, 0));
    assert_eq!(cleanup.deallocations, 3); // two payloads plus pinned test future
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    reservations::run(receiver, bytes, assignment, library);
    println!("actual Root specialized attempt storage: {bytes_used} payload bytes in two allocations under explicit 16384-byte construction ceiling; {future_bytes}-byte CPU pending future (separate input/context); late capacity and both allocator failures release storage before provider transfer; pending cancellation releases both allocations");
}

#[path = "root_reservations.rs"]
mod reservations;
