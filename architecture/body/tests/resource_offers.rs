use conduit_body::{
    Body, BodyResourceAllowance, BodyResourceEnvelope, BodyResourceEnvelopeError, PartId,
    MAX_BODY_RESOURCE_ALLOWANCES,
};
use conduit_core::{
    compute_reservation, compute_resource_offer, compute_resource_requirement, resource_offer,
    resource_requirement, ArchitectureBaseId, ArchitectureBaseKind, BootId, CheckedFormId,
    ComputeDomainId, ComputePerformanceClassId, ComputePoolContract, ComputeServiceGuarantee,
    ComputeTopologyGroup, ComputeTopologyGroupId, ComputeTopologyRequirement, HostAdvertisement,
    HostId, HostProfileId, OfferGeneration, ResourceBinding, ResourceHealth, ResourceObservation,
    SignId, SourceDocumentId, PROTOCOL_VERSION,
};

fn body_and_part() -> (conduit_body::BodyId, PartId) {
    let body = Body::born(
        SourceDocumentId::from("source"),
        CheckedFormId::from("checked"),
        1,
        SignId::from("born"),
    )
    .unwrap();
    let part = PartId::bind(&body.body_id, "durable-host", 1).unwrap();
    (body.body_id, part)
}

fn host(profile: &str, resources: Vec<conduit_core::ResourceOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(profile),
        boot_id: BootId::from(format!("{profile}/boot")),
        offer_generation: OfferGeneration(7),
        profile: HostProfileId::from(profile),
        resources,
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}

fn allowance(maximum_units: u32) -> BodyResourceAllowance {
    BodyResourceAllowance {
        pool_id: "execution".into(),
        class_id: "test/execution".into(),
        maximum_units,
    }
}

#[test]
fn browser_workstation_and_constrained_hosts_have_distinct_allowances() {
    let (body_id, _) = body_and_part();
    let mut identities = Vec::new();
    for (index, (profile, machine, host_units, body_units)) in [
        ("browser-page", 12, 4, 2),
        ("std-workstation", 32, 32, 24),
        ("constrained", 2, 1, 1),
    ]
    .into_iter()
    .enumerate()
    {
        let part_id = PartId::bind(&body_id, profile, index as u64).unwrap();
        assert!(body_units <= host_units && host_units <= machine);
        let advertisement = host(
            profile,
            vec![resource_offer("execution", "test/execution", host_units)],
        );
        let envelope = BodyResourceEnvelope::new(
            body_id.clone(),
            part_id.clone(),
            &advertisement,
            vec![allowance(body_units)],
        )
        .unwrap();
        assert_eq!(advertisement.resources[0].capacity_units, host_units);
        assert_eq!(envelope.allowances()[0].maximum_units, body_units);
        identities.push(envelope.envelope_id().clone());
    }
    assert!(identities.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn topology_bearing_eight_lane_host_pool_can_allow_body_only_two() {
    let (body_id, part_id) = body_and_part();
    let contract = ComputePoolContract {
        service_guarantee: ComputeServiceGuarantee::Reserved,
        architecture_base_id: ArchitectureBaseId::from("workstation/cpu"),
        architecture_base_kind: ArchitectureBaseKind::HostedOs,
        topology_groups: vec![ComputeTopologyGroup {
            group_id: ComputeTopologyGroupId::from("cache-cluster"),
            lane_capacity: 8,
            numa_domain: Some(ComputeDomainId::from("numa-0")),
            cache_domain: Some(ComputeDomainId::from("cache-0")),
            performance_class: Some(ComputePerformanceClassId::from("performance")),
            nominal_clock_hz: Some(3_000_000_000),
        }],
    };
    let advertisement = host(
        "std-workstation",
        vec![compute_resource_offer(
            "execution",
            "test/execution",
            8,
            contract.clone(),
        )],
    );
    let envelope =
        BodyResourceEnvelope::new(body_id, part_id, &advertisement, vec![allowance(2)]).unwrap();
    assert_eq!(advertisement.resources[0].capacity_units, 8);
    assert_eq!(advertisement.resources[0].compute, Some(contract));
    assert_eq!(envelope.allowances()[0].maximum_units, 2);

    let requirement = compute_resource_requirement(
        "test/execution",
        2,
        2,
        2,
        ComputeServiceGuarantee::Reserved,
        Some(ComputeTopologyRequirement {
            same_numa_domain: true,
            same_cache_domain: true,
            performance_class: Some(ComputePerformanceClassId::from("performance")),
        }),
    );
    let reservation = compute_reservation(&requirement, &advertisement.resources[0], 8).unwrap();
    let binding = ResourceBinding {
        content: None,
        pool_id: "execution".into(),
        class_id: "test/execution".into(),
        units: reservation.selected_lanes,
        protected: None,
        compute: Some(reservation),
    };
    let observation = observation(&advertisement, 6, 2);
    assert_eq!(
        envelope.validates_reservation(&requirement, &binding, &advertisement, &observation,),
        Ok(())
    );
}

#[test]
fn envelope_identity_tracks_part_policy_and_host_epoch() {
    let (body_id, part_id) = body_and_part();
    let advertisement = host(
        "workstation",
        vec![resource_offer("execution", "test/execution", 8)],
    );
    let four = BodyResourceEnvelope::new(
        body_id.clone(),
        part_id.clone(),
        &advertisement,
        vec![allowance(4)],
    )
    .unwrap();
    let repeated = BodyResourceEnvelope::new(
        body_id.clone(),
        part_id.clone(),
        &advertisement,
        vec![allowance(4)],
    )
    .unwrap();
    let two =
        BodyResourceEnvelope::new(body_id, part_id, &advertisement, vec![allowance(2)]).unwrap();
    assert_eq!(four.envelope_id(), repeated.envelope_id());
    assert_ne!(four.envelope_id(), two.envelope_id());
}

#[test]
fn original_requirement_allowance_and_current_observation_all_constrain_binding() {
    let (body_id, part_id) = body_and_part();
    let advertisement = host(
        "workstation",
        vec![resource_offer("execution", "test/execution", 8)],
    );
    let envelope =
        BodyResourceEnvelope::new(body_id, part_id, &advertisement, vec![allowance(3)]).unwrap();
    let requirement = resource_requirement("test/execution", 2);
    let binding = |units| ResourceBinding {
        content: None,
        pool_id: "execution".into(),
        class_id: "test/execution".into(),
        units,
        protected: None,
        compute: None,
    };
    assert_eq!(
        envelope.validates_reservation(
            &requirement,
            &binding(2),
            &advertisement,
            &observation(&advertisement, 7, 1),
        ),
        Ok(())
    );
    assert_eq!(
        envelope.validates_reservation(
            &requirement,
            &binding(4),
            &advertisement,
            &observation(&advertisement, 7, 1),
        ),
        Err(BodyResourceEnvelopeError::ReservationExceedsAllowance)
    );
    assert_eq!(
        envelope.validates_reservation(
            &requirement,
            &binding(2),
            &advertisement,
            &observation(&advertisement, 1, 7),
        ),
        Err(BodyResourceEnvelopeError::ReservationUnavailable)
    );
}

#[test]
fn envelope_is_finite_and_cannot_enlarge_host_offer() {
    let (body_id, part_id) = body_and_part();
    let advertisement = host(
        "browser",
        vec![resource_offer("execution", "test/execution", 4)],
    );
    assert_eq!(
        BodyResourceEnvelope::new(
            body_id.clone(),
            part_id.clone(),
            &advertisement,
            vec![allowance(5)],
        ),
        Err(BodyResourceEnvelopeError::EnlargesHostOffer)
    );
    assert_eq!(
        BodyResourceEnvelope::new(
            body_id,
            part_id,
            &advertisement,
            vec![allowance(1); MAX_BODY_RESOURCE_ALLOWANCES + 1],
        ),
        Err(BodyResourceEnvelopeError::CapacityExceeded)
    );
    let (other_body, other_part) = body_and_part();
    assert_eq!(
        BodyResourceEnvelope::new(
            other_body,
            other_part,
            &advertisement,
            vec![allowance(2), allowance(4)],
        ),
        Err(BodyResourceEnvelopeError::DuplicatePool)
    );
}

#[test]
fn omitted_pool_and_stale_offer_generation_refuse_even_when_host_has_capacity() {
    let (body_id, part_id) = body_and_part();
    let advertisement = host(
        "workstation",
        vec![
            resource_offer("execution", "test/execution", 8),
            resource_offer("memory", "test/memory", 32),
        ],
    );
    let envelope =
        BodyResourceEnvelope::new(body_id, part_id, &advertisement, vec![allowance(4)]).unwrap();
    let omitted_requirement = resource_requirement("test/memory", 1);
    let omitted_binding = ResourceBinding {
        content: None,
        pool_id: "memory".into(),
        class_id: "test/memory".into(),
        units: 1,
        protected: None,
        compute: None,
    };
    let mut memory_observation = observation(&advertisement, 32, 0);
    memory_observation.pool_id = "memory".into();
    memory_observation.class_id = "test/memory".into();
    assert_eq!(
        envelope.validates_reservation(
            &omitted_requirement,
            &omitted_binding,
            &advertisement,
            &memory_observation,
        ),
        Err(BodyResourceEnvelopeError::InvalidReservation)
    );

    let requirement = resource_requirement("test/execution", 2);
    let binding = ResourceBinding {
        content: None,
        pool_id: "execution".into(),
        class_id: "test/execution".into(),
        units: 2,
        protected: None,
        compute: None,
    };
    let mut changed_offer = advertisement.clone();
    changed_offer.offer_generation = OfferGeneration(advertisement.offer_generation.0 + 1);
    assert_eq!(
        envelope.validates_reservation(
            &requirement,
            &binding,
            &changed_offer,
            &observation(&changed_offer, 8, 0),
        ),
        Err(BodyResourceEnvelopeError::StaleObservation)
    );
}

fn observation(
    host: &HostAdvertisement,
    unreserved_units: u32,
    utilized_units: u32,
) -> ResourceObservation {
    ResourceObservation {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        pool_id: "execution".into(),
        class_id: "test/execution".into(),
        health: ResourceHealth::Ready,
        unreserved_units,
        utilized_units,
        sign_id: "resource-now".into(),
    }
}
