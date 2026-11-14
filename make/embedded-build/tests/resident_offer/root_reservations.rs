//! Actual Root storage and the existing Core resource owner share one attempt.
use super::*;

pub fn run(
    receiver: &Receiver, bytes: &[u8], assignment: &crate::support::Assignment,
    library: &mut crate::library::Initialized,
) {
    let binding = &assignment.model.fragments[0].placements[0].resources[0];
    let mut actual = receiver.actual.clone();
    actual.resources.push(c::resource_offer(binding.pool_id.as_str(), binding.class_id.as_str(), 5000));
    // Private computation readiness is independent of other declared pools;
    // their presence alone supplies no readiness evidence for native IO.
    actual.resources.push(c::resource_offer("fixture/other", "fixture/other-class", 1));
    let mut owner = c::ResourceAdmissionOwner::new(actual);
    let before = owner.clone();
    let original = library.offers().map(|o| o.unwrap().as_ptr());
    macro_rules! prepare { () => { crate::root_preparer::prepare_workset_storage(
        library, Some(&policy()), receiver.context(),
        receiver.command_with_contract(bytes, HOST_PREPARED_WORKSET_CONTRACT),
        assignment.maxima, 16384,
    ).unwrap() }; }
    let stored = prepare!();
    let observe = |maximum| crate::root_preparer::observe_private_work(
        &stored, &owner, binding.pool_id.as_str(), 5000, 1, maximum,
    );
    let (evidence, observation_cost) = crate::probe::observe(|| observe(1024));
    let evidence = evidence.unwrap();
    assert!(observation_cost.requested_bytes <= evidence.owned_bytes);
    assert_eq!(observation_cost.allocations, 5);
    for nth in 1..=observation_cost.allocations {
        let (refused, cost) = crate::probe::observe_refusing(nth, || observe(1024));
        assert!(matches!(refused, Err(DelegatedResourceRefusal::Allocation)));
        assert_eq!(cost.refused_allocations, 1);
        assert_eq!(cost.allocations, cost.deallocations);
    }
    assert!(matches!(observe(evidence.owned_bytes - 1),
        Err(DelegatedResourceRefusal::PreparationCapacity)));
    drop(stored);
    let budget = 16384 - evidence.owned_bytes;
    let observed = evidence.value;
    // Missing and insufficient observations drop the already acquired kernels.
    assert!(matches!(prepare!().reserve(&mut owner, &[], budget), Err(DelegatedResourceRefusal::Admission(
        c::ResourceAdmissionRefusal::MissingObservation,
    ))));
    assert_eq!(owner, before);
    let mut low = observed.clone();
    low.unreserved_units = 0;
    assert!(matches!(prepare!().reserve(&mut owner, &[low], budget), Err(DelegatedResourceRefusal::Admission(
        c::ResourceAdmissionRefusal::Overcommitted,
    ))));
    assert_eq!(owner, before);
    let stored = prepare!();
    let storage_bytes = stored.storage_bytes();
    let (reserved, cost) = crate::probe::observe(||
        stored.reserve(&mut owner, core::slice::from_ref(&observed), budget));
    assert_eq!(reserved.as_ref().unwrap().admissions().len(), 2);
    let preparation_bytes = reserved.as_ref().unwrap().preparation_bytes();
    assert!(cost.peak_bytes + storage_bytes <= preparation_bytes);
    drop(reserved);
    assert_eq!(owner, before);
    for nth in 1..=cost.allocations {
        let mut owner = before.clone();
        let stored = prepare!();
        let (result, failure) = crate::probe::observe_refusing(nth, ||
            stored.reserve(&mut owner, core::slice::from_ref(&observed), budget));
        assert!(matches!(result, Err(DelegatedResourceRefusal::Allocation |
            DelegatedResourceRefusal::Admission(c::ResourceAdmissionRefusal::Allocation))));
        assert_eq!(failure.refused_allocations, 1);
        drop(result);
        assert_eq!(owner, before);
    }
    assert!(matches!(prepare!().reserve(&mut owner,
        core::slice::from_ref(&observed), preparation_bytes - 1),
        Err(DelegatedResourceRefusal::PreparationCapacity)));
    assert_eq!(owner, before);
    let future = async {
        let reserved = prepare!().reserve_observed(&mut owner, binding.pool_id.as_str(), 5000, 2, 16384).unwrap();
        assert_eq!(reserved.admissions().len(), 2);
        assert!(reserved.admissions().iter().all(|r| r.plan_id.as_str() ==
            assignment_body_plan(bytes)));
        core::future::pending::<()>().await;
        drop(reserved);
    };
    let future_bytes = core::mem::size_of_val(&future);
    assert!(future_bytes < 2048);
    let mut future = Box::pin(future);
    use core::future::Future;
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    assert!(future.as_mut().poll(&mut cx).is_pending());
    let ((), cleanup) = crate::probe::observe(|| drop(future));
    assert_eq!((cleanup.allocations, cleanup.reallocations), (0, 0));
    assert_eq!(owner, before);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    binding::run(
        receiver, bytes, assignment, library, &mut owner, binding.pool_id.as_str(),
    );
    // The real constructor can retire its feasibility storage and hand only
    // the original attribution to a longer-lived, independently owned buffer.
    let mut pending = HostCommandInput::<4096>::new();
    pending.writable().unwrap()[..bytes.len()].copy_from_slice(bytes);
    let stamp = prepare!().reserve_observed(
        &mut owner, binding.pool_id.as_str(), 5000, 3, 16384,
    ).unwrap().release_for_retention();
    assert_eq!(owner, before);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    let ((), retention) = crate::probe::observe(|| pending.retain(stamp).unwrap());
    assert_eq!((retention.allocations, retention.reallocations), (0, 0));
    let command = pending.reborrow(&receiver.member, &receiver.actual,
        &receiver.bootstrap.owner, HOST_PREPARED_WORKSET_CONTRACT, 11).unwrap();
    assert_eq!(command.payload(), bytes);
    assert!(pending.reborrow(&receiver.member, &receiver.actual,
        &receiver.bootstrap.owner, HOST_PREPARED_WORKSET_CONTRACT, 20).is_err());
    println!("actual Root attempt reserves both exact fragments through Core within {preparation_bytes} kernel/bookkeeping requested-byte bound plus {} observation bytes; missing/low observations, one-byte shortage and all {} reservation allocations refuse; {future_bytes}-byte CPU pending attempt; cancellation frees kernels and exact reservations without allocating or lending providers", evidence.owned_bytes, cost.allocations);
}

fn assignment_body_plan(bytes: &[u8]) -> &str {
    // CBWSET01, count, first document length; inspect original approved metadata.
    let (m, _): (BodyFragmentApprovalMetadata<'_>, _) =
        postcard::take_from_bytes(&bytes[14..]).unwrap();
    m.body_plan
}

#[path = "root_binding.rs"]
mod binding;
