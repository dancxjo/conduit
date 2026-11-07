//! Original command expiry is not evidence of native retirement.
use super::*;

#[test]
fn expired_idle_preparation_reclaims_storage_and_issues_a_new_challenge() {
    NOW.with(|n| n.set(100));
    let mut member = member();
    let mut workspace = HostCommandInput::<LIMIT>::new();
    let mut first = Link::new(Mode::Receipt, 1000);
    run(member.serve_preparation(&mut first, &mut workspace)).unwrap();
    let deadline = workspace.expires_at_millis().unwrap();
    let original = first.nonce;
    let sequence = member.freshness;
    NOW.with(|n| n.set(deadline - 1));
    let (reclaimed, cost) = probe::observe(|| member.reclaim_expired_preparation(&mut workspace));
    assert!(!reclaimed);
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(workspace.payload().unwrap(), first.payload);
    NOW.with(|n| n.set(deadline));
    let (reclaimed, cost) = probe::observe(|| member.reclaim_expired_preparation(&mut workspace));
    assert!(reclaimed);
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(!workspace.is_retained());
    assert!(workspace.writable().unwrap().iter().all(|b| *b == 0));
    assert_eq!(member.freshness, sequence);
    let mut next = Link::new(Mode::Receipt, 1000);
    run(member.serve_preparation(&mut next, &mut workspace)).unwrap();
    assert_ne!(next.nonce, original);
    assert_eq!(member.freshness, sequence + 1);
    assert_eq!(workspace.expires_at_millis(), Some(deadline + 10_000));
    NOW.with(|n| n.set(deadline + 10_000));
    let mut third = Link::new(Mode::Receipt, 1000);
    // Exercise reclamation through the actual entry, without calling the helper.
    run(member.serve_preparation(&mut third, &mut workspace)).unwrap();
    assert_ne!(third.nonce, next.nonce);
    assert_eq!(member.freshness, sequence + 2);
}

#[test]
fn busy_or_unretired_inventory_preserves_expired_input() {
    NOW.with(|n| n.set(100));
    let mut member = member();
    let mut workspace = HostCommandInput::<LIMIT>::new();
    let mut first = Link::new(Mode::Receipt, 1000);
    run(member.serve_preparation(&mut first, &mut workspace)).unwrap();
    NOW.with(|n| n.set(workspace.expires_at_millis().unwrap()));
    for withdrawn in [false, true] {
        resident_preparation::BUSY.with(|v| v.set(!withdrawn));
        resident_preparation::WITHDRAWN.with(|v| v.set(withdrawn));
        let (reclaimed, cost) =
            probe::observe(|| member.reclaim_expired_preparation(&mut workspace));
        assert!(!reclaimed);
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
        assert_eq!(workspace.payload().unwrap(), first.payload);
    }
    resident_preparation::BUSY.with(|v| v.set(false));
    resident_preparation::WITHDRAWN.with(|v| v.set(false));
    assert!(member.reclaim_expired_preparation(&mut workspace));
}

#[test]
fn an_outstanding_resource_admission_prevents_expiry_reclamation() {
    NOW.with(|n| n.set(100));
    let mut member = member();
    let mut workspace = HostCommandInput::<LIMIT>::new();
    let mut first = Link::new(Mode::Receipt, 1000);
    run(member.serve_preparation(&mut first, &mut workspace)).unwrap();
    let ad = member.resources.host().clone();
    let observed = c::ResourceObservation {
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        pool_id: ad.resources[0].pool_id.clone(),
        class_id: ad.resources[0].class_id.clone(),
        offer_generation: ad.offer_generation,
        health: c::ResourceHealth::Ready,
        unreserved_units: 1,
        utilized_units: 0,
        sign_id: "fixture/ready".into(),
    };
    member
        .resources
        .admit(
            c::ResourceAdmissionRequest {
                plan_id: "fixture/held".into(),
                placement_id: "fixture/held-placement".into(),
                host_id: ad.host_id,
                boot_id: ad.boot_id,
                offer_generation: ad.offer_generation,
                items: vec![c::ResourceAdmissionItem {
                    requirement: c::resource_requirement(c::COOPERATIVE_STEP_WORK_CLASS_ID, 1),
                    binding: c::ResourceBinding {
                        pool_id: ad.resources[0].pool_id.clone(),
                        class_id: ad.resources[0].class_id.clone(),
                        units: 1,
                        protected: None,
                        compute: None,
                        content: None,
                    },
                }],
            },
            &[observed],
        )
        .unwrap();
    NOW.with(|n| n.set(workspace.expires_at_millis().unwrap()));
    let before = member.resources.clone();
    let (reclaimed, cost) = probe::observe(|| member.reclaim_expired_preparation(&mut workspace));
    assert!(!reclaimed);
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(workspace.payload().unwrap(), first.payload);
    assert_eq!(member.resources, before);
    member
        .resources
        .release(&"fixture/held".into(), &"fixture/held-placement".into())
        .unwrap();
    assert!(member.reclaim_expired_preparation(&mut workspace));
}
