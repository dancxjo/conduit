//! Actual Root storage and the existing Core resource owner share one attempt.
use super::*;

pub fn run(
    receiver: &Receiver, bytes: &[u8], assignment: &crate::support::Assignment,
    library: &mut crate::library::Initialized,
) {
    let binding = &assignment.model.fragments[0].placements[0].resources[0];
    let mut actual = receiver.actual.clone();
    actual.resources.push(c::resource_offer(binding.pool_id.as_str(), binding.class_id.as_str(), 5000));
    let observed = c::ResourceObservation {
        host_id: actual.host_id.clone(), boot_id: actual.boot_id.clone(),
        offer_generation: actual.offer_generation, pool_id: binding.pool_id.clone(),
        class_id: binding.class_id.clone(), health: c::ResourceHealth::Ready,
        unreserved_units: 5000, utilized_units: 0, sign_id: "fixture/root-ready".into(),
    };
    let mut owner = c::ResourceAdmissionOwner::new(actual);
    let before = owner.clone();
    let original = library.offers().map(|o| o.unwrap().as_ptr());
    macro_rules! prepare { () => { crate::root_preparer::prepare_workset_storage(
        library, Some(&policy()), receiver.context(),
        receiver.command_with_contract(bytes, HOST_PREPARED_WORKSET_CONTRACT),
        assignment.maxima, 16384,
    ).unwrap() }; }
    // Missing and insufficient observations drop the already acquired kernels.
    assert!(matches!(prepare!().reserve(&mut owner, &[]), Err(DelegatedResourceRefusal::Admission(
        c::ResourceAdmissionRefusal::MissingObservation,
    ))));
    assert_eq!(owner, before);
    let mut low = observed.clone();
    low.unreserved_units = 0;
    assert!(matches!(prepare!().reserve(&mut owner, &[low]), Err(DelegatedResourceRefusal::Admission(
        c::ResourceAdmissionRefusal::Overcommitted,
    ))));
    assert_eq!(owner, before);
    let stored = prepare!();
    let (reserved, cost) = crate::probe::observe(||
        stored.reserve(&mut owner, core::slice::from_ref(&observed)));
    assert_eq!(reserved.as_ref().unwrap().admissions().len(), 2);
    drop(reserved);
    assert_eq!(owner, before);
    for nth in 1..=cost.allocations {
        let mut owner = before.clone();
        let stored = prepare!();
        let (result, failure) = crate::probe::observe_refusing(nth, ||
            stored.reserve(&mut owner, core::slice::from_ref(&observed)));
        assert!(matches!(result, Err(DelegatedResourceRefusal::Allocation |
            DelegatedResourceRefusal::Admission(c::ResourceAdmissionRefusal::Allocation))));
        assert_eq!(failure.refused_allocations, 1);
        drop(result);
        assert_eq!(owner, before);
    }
    let mut future = Box::pin(async {
        let reserved = prepare!().reserve(&mut owner, core::slice::from_ref(&observed)).unwrap();
        assert_eq!(reserved.admissions().len(), 2);
        assert!(reserved.admissions().iter().all(|r| r.plan_id.as_str() ==
            assignment_body_plan(bytes)));
        core::future::pending::<()>().await;
        drop(reserved);
    });
    use core::future::Future;
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    assert!(future.as_mut().poll(&mut cx).is_pending());
    let ((), cleanup) = crate::probe::observe(|| drop(future));
    assert_eq!((cleanup.allocations, cleanup.reallocations), (0, 0));
    assert_eq!(owner, before);
    assert_eq!(library.offers().map(|o| o.unwrap().as_ptr()), original);
    println!("actual Root attempt reserves both exact fragments through Core; missing/low observations and all {} reservation allocations refuse; pending cancellation frees kernels and exact reservations without allocating or lending providers", cost.allocations);
}

fn assignment_body_plan(bytes: &[u8]) -> &str {
    // CBWSET01, count, first document length; inspect original approved metadata.
    let (m, _): (BodyFragmentApprovalMetadata<'_>, _) =
        postcard::take_from_bytes(&bytes[14..]).unwrap();
    m.body_plan
}
