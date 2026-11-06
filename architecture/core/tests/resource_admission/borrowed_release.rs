use super::*;

#[test]
fn borrowed_release_keeps_unrelated_reservations_and_allocates_nothing() {
    let host = host();
    let mut owner = ResourceAdmissionOwner::new(host.clone());
    let observations = [observation(&host, 16, "sign/borrowed-release")];
    let first = batch::request(&host, "first", 1);
    let other = batch::request(&host, "other", 1);
    owner.admit(first.clone(), &observations).unwrap();
    owner.admit(other.clone(), &observations).unwrap();
    for request in [&first, &other] {
        owner
            .assign_compute_lanes(
                &request.plan_id,
                &request.placement_id,
                "play/borrowed".into(),
                &["lane/0".into()],
            )
            .unwrap();
    }
    let before = owner.clone();
    let (refused, cost) = probe::observe(|| {
        owner.release_borrowed(
            first.plan_id.as_str(),
            other.placement_id.as_str(),
            ResourceReleaseCause::Cancelled,
        )
    });
    assert_eq!(refused, Err(ResourceAdmissionRefusal::UnknownAdmission));
    assert_eq!(owner, before);
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let (released, cost) = probe::observe(|| {
        owner
            .release_borrowed(
                first.plan_id.as_str(),
                first.placement_id.as_str(),
                ResourceReleaseCause::Cancelled,
            )
            .unwrap()
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(released.cause, ResourceReleaseCause::Cancelled);
    assert_eq!(released.admission.plan_id, first.plan_id);
    assert_eq!(owner.admissions().len(), 1);
    assert_eq!(owner.admissions()[0].plan_id, other.plan_id);
    assert_eq!(owner.assignments().len(), 1);
    assert_eq!(owner.assignments()[0].plan_id, other.plan_id);
}
