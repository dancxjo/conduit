use super::*;

#[test]
fn pending_preparation_cancellation_releases_only_its_atomic_batches() {
    let host = host();
    let observed = [observation(&host, 16, "sign/preparation")];
    let mut owner = ResourceAdmissionOwner::new(host.clone());
    let existing = batch::request(&host, "existing", 4);
    owner.admit(existing.clone(), &observed).unwrap();
    owner
        .assign_compute_lanes(
            &existing.plan_id,
            &existing.placement_id,
            "play/existing".into(),
            &["lane/existing".into()],
        )
        .unwrap();
    let before = owner.clone();
    let mut future = Box::pin(async {
        let mut scope = owner.preparation_scope();
        assert_eq!(scope.host(), &host);
        scope
            .admit_batch(vec![batch::request(&host, "first", 8)], &observed)
            .unwrap();
        assert_eq!(scope.admissions().len(), 1);
        assert_eq!(
            scope.admit_batch(vec![batch::request(&host, "late", 8)], &observed,),
            Err(ResourceAdmissionRefusal::Overcommitted)
        );
        assert_eq!(scope.admissions().len(), 1);
        scope
            .admit_batch(vec![batch::request(&host, "second", 4)], &observed)
            .unwrap();
        assert_eq!(scope.admissions().len(), 2);
        core::future::pending::<()>().await;
        drop(scope);
    });
    use core::future::Future;
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    assert!(future.as_mut().poll(&mut cx).is_pending());
    let ((), cleanup) = probe::observe(|| drop(future));
    assert_eq!(cleanup.allocations, 0);
    assert_eq!(cleanup.reallocations, 0);
    assert_eq!(owner, before);

    let ((), empty) = probe::observe(|| drop(owner.preparation_scope()));
    assert_eq!(empty.allocations, 0);
    assert_eq!(empty.reallocations, 0);
    assert_eq!(owner, before);
}
