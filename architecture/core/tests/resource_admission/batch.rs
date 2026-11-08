use super::*;

pub(super) fn request(host: &HostAdvertisement, id: &str, units: u32) -> ResourceAdmissionRequest {
    ResourceAdmissionRequest {
        plan_id: format!("plan/{id}").into(),
        placement_id: format!("placement/{id}").into(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        items: vec![item(host, units, units, units)],
    }
}

#[test]
fn batch_admission_is_atomic_and_retains_exact_per_plan_reservations() {
    let host = host();
    let observed = [observation(&host, 16, "sign/batch")];
    let mut owner = ResourceAdmissionOwner::new(host.clone());
    let existing = request(&host, "existing", 4);
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
    let first = request(&host, "first", 8);
    let second = request(&host, "second", 8);
    assert_eq!(
        owner.admit_batch(vec![first.clone(), second], &observed),
        Err(ResourceAdmissionRefusal::Overcommitted)
    );
    assert_eq!(owner, before);

    let mut stale = request(&host, "stale", 1);
    stale.boot_id = "boot/stale".into();
    assert_eq!(
        owner.admit_batch(vec![first.clone(), stale], &observed),
        Err(ResourceAdmissionRefusal::StaleOffer)
    );
    assert_eq!(owner, before);
    assert_eq!(
        owner.admit_batch(vec![first.clone(), first.clone()], &observed),
        Err(ResourceAdmissionRefusal::DuplicatePlanPlacement)
    );
    assert_eq!(owner, before);
    assert_eq!(
        owner.admit_batch(Vec::new(), &observed),
        Err(ResourceAdmissionRefusal::Empty)
    );
    assert_eq!(owner, before);

    let second = request(&host, "second", 4);
    let oversized = (0..64)
        .map(|index| request(&host, &format!("overflow-{index}"), 1))
        .collect();
    assert_eq!(
        owner.admit_batch(oversized, &observed),
        Err(ResourceAdmissionRefusal::CapacityExceeded)
    );
    assert_eq!(owner, before);
    let mut empty = request(&host, "empty", 1);
    empty.items.clear();
    assert_eq!(
        owner.admit_batch(vec![first.clone(), empty], &observed),
        Err(ResourceAdmissionRefusal::Empty)
    );
    assert_eq!(owner, before);
    let admitted = owner
        .admit_batch(vec![first.clone(), second.clone()], &observed)
        .unwrap();
    assert_eq!(admitted.len(), 2);
    assert_eq!(admitted[0].plan_id, first.plan_id);
    assert_eq!(admitted[1].plan_id, second.plan_id);
    assert_eq!(
        admitted[0].observation_sign_ids,
        [observed[0].sign_id.clone()]
    );
    assert_eq!(owner.assignments(), before.assignments());
    owner
        .release_for(
            &first.plan_id,
            &first.placement_id,
            ResourceReleaseCause::Completed,
        )
        .unwrap();
    owner
        .release_for(
            &second.plan_id,
            &second.placement_id,
            ResourceReleaseCause::Completed,
        )
        .unwrap();
    assert_eq!(owner, before);
}

#[test]
fn batch_preparation_does_not_copy_unrelated_host_inventory() {
    fn measured(extra_pools: usize) -> probe::Observation {
        let mut host = host();
        for index in 0..extra_pools {
            host.resources.push(conduit_core::resource_offer(
                &format!("pool/unrelated/{index}"),
                "fixture/unrelated",
                1,
            ));
        }
        let observed = [observation(&host, 16, "sign/batch-cost")];
        let mut owner = ResourceAdmissionOwner::new(host.clone());
        let existing = request(&host, "existing", 4);
        owner.admit(existing, &observed).unwrap();
        let requests = vec![request(&host, "first", 4), request(&host, "second", 4)];
        let ((), cost) = probe::observe(|| {
            assert_eq!(owner.admit_batch(requests, &observed).unwrap().len(), 2);
        });
        assert_eq!(owner.admissions().len(), 3);
        assert_eq!(owner.host(), &host);
        cost
    }
    let small = measured(0);
    let large = measured(128);
    eprintln!("batch preparation: small {small:?}; 128 extra pools {large:?}");
    assert_eq!(large.allocations, small.allocations);
    assert_eq!(large.reallocations, small.reallocations);
    assert_eq!(large.requested_bytes, small.requested_bytes);
    assert_eq!(large.peak_bytes, small.peak_bytes);
}

#[test]
fn empty_owner_adopts_one_exact_staging_table_after_full_validation() {
    let host = host();
    let observed = [observation(&host, 16, "sign/empty-batch")];
    let mut owner = ResourceAdmissionOwner::new(host.clone());
    let inputs = vec![request(&host, "first", 4), request(&host, "second", 4)];
    let ((), cost) = probe::observe(|| {
        assert_eq!(owner.admit_batch(inputs, &observed).unwrap().len(), 2);
    });
    // One outer table, one evidence table and one copied Sign per request.
    // All request IDs and item backing transfer without copying.
    assert_eq!(cost.allocations, 5);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(
        cost.requested_bytes,
        2 * core::mem::size_of::<conduit_core::ResourceAdmission>()
            + 2 * core::mem::size_of::<SignId>()
            + 2 * observed[0].sign_id.as_str().len()
    );
    assert_eq!(
        owner.admissions()[0].placement_id.as_str(),
        "placement/first"
    );
    assert_eq!(
        owner.admissions()[1].placement_id.as_str(),
        "placement/second"
    );
    assert!(owner.assignments().is_empty());

    let mut empty = ResourceAdmissionOwner::new(host);
    let before = empty.clone();
    let inputs = vec![
        request(empty.host(), "first", 8),
        request(empty.host(), "late", 9),
    ];
    assert_eq!(
        empty.admit_batch(inputs, &observed),
        Err(ResourceAdmissionRefusal::Overcommitted)
    );
    assert_eq!(empty, before);
}
