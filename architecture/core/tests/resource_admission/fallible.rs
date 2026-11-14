//! Each allocation boundary must refuse without changing the current owner.
use super::*;
use conduit_core::{
    bind_resource_content, kind_id, resource_offer, resource_requirement, HostBaseId,
    ResourceAccessMode, ResourceContentOffer, ResourceContentRequirement, ResourceRetention,
    ResourceSemanticIdentity, ResourceSharing, ResourceVersionIdentity,
};

fn fixture(
    content: bool,
) -> (
    ResourceAdmissionOwner,
    Vec<ResourceObservation>,
    Vec<ResourceAdmissionRequest>,
) {
    let mut host = host();
    let mut observed = vec![observation(&host, 16, "sign/compute")];
    let items = if content {
        let contract = ResourceContentRequirement {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            version: ResourceVersionIdentity::from_digest([2; 32]),
            content_profile: kind_id("frame/rgba@1"),
            maximum_bytes: 1024,
            maximum_items: 256,
            retention: ResourceRetention::Play,
            sharing: ResourceSharing::SingleWriterPublished,
            access: ResourceAccessMode::WriteCandidatePublish,
            generation_slots: 1,
            reader_leases: 3,
            publication_slots: 1,
            sensitive: false,
        };
        let mut pool = resource_offer("pool/content", "frame", 8);
        pool.content = Some(ResourceContentOffer {
            contract: contract.clone(),
            owner_host: host.host_id.clone(),
            owner_boot: host.boot_id.clone(),
            base_id: HostBaseId::from("base/arena"),
            residence_profile: kind_id("arena@1"),
        });
        let mut requirement = resource_requirement("frame", 1);
        requirement.content = Some(ResourceContentRequirement {
            access: ResourceAccessMode::ReadPublished,
            publication_slots: 0,
            ..contract
        });
        let binding = ResourceBinding {
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            units: 1,
            protected: None,
            compute: None,
            content: bind_resource_content(&requirement, &pool, &host.host_id, &host.boot_id)
                .unwrap(),
        };
        observed.push(ResourceObservation {
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            sign_id: "sign/content".into(),
            unreserved_units: 8,
            ..observed[0].clone()
        });
        host.resources.push(pool);
        // Mixed classes and duplicate observations exercise borrowed content
        // matching and sorted/deduplicated evidence without scratch allocation.
        vec![
            ResourceAdmissionItem {
                requirement,
                binding,
            },
            item(&host, 1, 1, 1),
            item(&host, 1, 1, 1),
        ]
    } else {
        vec![item(&host, 1, 1, 1), item(&host, 1, 1, 1)]
    };
    let requests = ["first", "second"]
        .map(|id| ResourceAdmissionRequest {
            plan_id: format!("plan/{id}").into(),
            placement_id: format!("place/{id}").into(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            items: items.clone(),
        })
        .into();
    let mut owner = ResourceAdmissionOwner::new(host.clone());
    admit(
        &mut owner,
        &host,
        "plan/existing",
        "place/existing",
        &observed[0],
        item(&host, 1, 1, 1),
    )
    .unwrap();
    owner
        .assign_compute_lanes(
            &"plan/existing".into(),
            &"place/existing".into(),
            "play/existing".into(),
            &["lane/existing".into()],
        )
        .unwrap();
    (owner, observed, requests)
}

#[test]
fn every_batch_allocation_refuses_atomically_including_borrowed_content_checks() {
    for content in [false, true] {
        let (before, observed, requests) = fixture(content);
        let mut owner = before.clone();
        let inputs = requests.clone();
        let (result, baseline) =
            probe::observe(|| owner.admit_batch(inputs, &observed).map(|r| r.len()));
        assert_eq!(result, Ok(2));
        assert!(baseline.allocation_attempts >= 4);
        for nth in 1..=baseline.allocation_attempts + 1 {
            let mut owner = before.clone();
            let inputs = requests.clone();
            let (result, cost) = probe::observe_refusing(nth, || {
                owner.admit_batch(inputs, &observed).map(|r| r.len())
            });
            if nth <= baseline.allocation_attempts {
                assert_eq!(
                    result,
                    Err(ResourceAdmissionRefusal::Allocation),
                    "request {nth}: {cost:?}"
                );
                assert_eq!(cost.refused_allocations, 1);
                assert_eq!(owner, before);
            } else {
                assert_eq!(result, Ok(2));
                assert_eq!(cost.refused_allocations, 0);
                assert_eq!(owner.assignments(), before.assignments());
                assert_eq!(
                    owner.admissions()[1].observation_sign_ids.len(),
                    if content { 2 } else { 1 }
                );
                assert!(owner.admissions()[1]
                    .observation_sign_ids
                    .windows(2)
                    .all(|pair| pair[0] < pair[1]));
            }
        }
    }
}

#[test]
fn every_single_admission_allocation_preserves_old_reservations_and_lanes() {
    let (before, observed, requests) = fixture(true);
    let mut owner = before.clone();
    let input = requests[0].clone();
    let (result, baseline) = probe::observe(|| owner.admit(input, &observed).map(|_| ()));
    assert_eq!(result, Ok(()));
    assert!(baseline.allocation_attempts >= 2);
    for nth in 1..=baseline.allocation_attempts {
        let mut owner = before.clone();
        let input = requests[0].clone();
        let (result, cost) =
            probe::observe_refusing(nth, || owner.admit(input, &observed).map(|_| ()));
        assert_eq!(
            result,
            Err(ResourceAdmissionRefusal::Allocation),
            "request {nth}: {cost:?}"
        );
        assert_eq!(cost.refused_allocations, 1);
        assert_eq!(owner, before);
    }
}

#[test]
fn every_empty_owner_batch_allocation_refuses_before_commit() {
    for content in [false, true] {
        let (existing, observed, requests) = fixture(content);
        let before = ResourceAdmissionOwner::new(existing.host().clone());
        let mut owner = before.clone();
        let inputs = requests.clone();
        let (result, baseline) =
            probe::observe(|| owner.admit_batch(inputs, &observed).map(|r| r.len()));
        assert_eq!(result, Ok(2));
        for nth in 1..=baseline.allocation_attempts + 1 {
            let mut owner = before.clone();
            let inputs = requests.clone();
            let (result, cost) = probe::observe_refusing(nth, || {
                owner.admit_batch(inputs, &observed).map(|r| r.len())
            });
            if nth <= baseline.allocation_attempts {
                assert_eq!(result, Err(ResourceAdmissionRefusal::Allocation));
                assert_eq!(cost.refused_allocations, 1);
                assert_eq!(owner, before);
            } else {
                assert_eq!(result, Ok(2));
                assert_eq!(cost.refused_allocations, 0);
                assert_eq!(owner.assignments(), before.assignments());
            }
        }
    }
}
