use conduit_core::{
    compute_reservation, compute_resource_offer, compute_resource_requirement, ActivePlayId,
    ArchitectureBaseId, ArchitectureBaseKind, BaseExecutionLaneId, BootId, ComputePoolContract,
    ComputeServiceGuarantee, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    PlacementId, PlanId, ResourceAdmissionItem, ResourceAdmissionOwner, ResourceAdmissionRefusal,
    ResourceAdmissionRequest, ResourceBinding, ResourceHealth, ResourceObservation,
    ResourceReleaseCause, SignId,
};

const CLASS: &str = "conduit.resource/compute/shared-lane@1";

#[path = "resource_admission/batch.rs"]
mod batch;

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: 1,
        host_id: HostId::from("host/compute-fixture"),
        boot_id: BootId::from("boot/compute-fixture/1"),
        offer_generation: OfferGeneration(7),
        profile: HostProfileId::from("conduit.host/compute-fixture@1"),
        resources: vec![compute_resource_offer(
            "pool/compute",
            CLASS,
            16,
            ComputePoolContract {
                service_guarantee: ComputeServiceGuarantee::Shared,
                architecture_base_id: ArchitectureBaseId::from("base/hosted-compute@1"),
                architecture_base_kind: ArchitectureBaseKind::HostedOs,
                topology_groups: vec![],
            },
        )],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}

fn observation(host: &HostAdvertisement, unreserved_units: u32, sign: &str) -> ResourceObservation {
    ResourceObservation {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        pool_id: host.resources[0].pool_id.clone(),
        class_id: host.resources[0].class_id.clone(),
        health: ResourceHealth::Ready,
        unreserved_units,
        utilized_units: 0,
        sign_id: SignId::from(sign),
    }
}

fn item(
    host: &HostAdvertisement,
    minimum: u32,
    preferred: u32,
    maximum: u32,
) -> ResourceAdmissionItem {
    item_with_available(host, minimum, preferred, maximum, preferred)
}

fn item_with_available(
    host: &HostAdvertisement,
    minimum: u32,
    preferred: u32,
    maximum: u32,
    available: u32,
) -> ResourceAdmissionItem {
    let requirement = compute_resource_requirement(
        CLASS,
        minimum,
        preferred,
        maximum,
        ComputeServiceGuarantee::Shared,
        None,
    );
    let reservation = compute_reservation(&requirement, &host.resources[0], available)
        .expect("fixture requirement fits");
    ResourceAdmissionItem {
        binding: ResourceBinding {
            content: None,
            pool_id: host.resources[0].pool_id.clone(),
            class_id: host.resources[0].class_id.clone(),
            units: reservation.selected_lanes,
            protected: None,
            compute: Some(reservation),
        },
        requirement,
    }
}

fn admit(
    owner: &mut ResourceAdmissionOwner,
    host: &HostAdvertisement,
    plan: &str,
    placement: &str,
    observation: &ResourceObservation,
    item: ResourceAdmissionItem,
) -> Result<(), ResourceAdmissionRefusal> {
    owner
        .admit(
            ResourceAdmissionRequest {
                plan_id: PlanId::from(plan),
                placement_id: PlacementId::from(placement),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                items: vec![item],
            },
            core::slice::from_ref(observation),
        )
        .map(|_| ())
}

#[test]
fn one_atomic_law_admits_scalable_multicore_contention_and_releases_exactly() {
    let host = host();
    let current = observation(&host, 16, "sign/capacity/1");
    let mut owner = ResourceAdmissionOwner::new(host.clone());

    admit(
        &mut owner,
        &host,
        "plan/a",
        "place/a",
        &current,
        item_with_available(&host, 2, 8, 8, 16),
    )
    .unwrap();
    admit(
        &mut owner,
        &host,
        "plan/b",
        "place/b",
        &current,
        item_with_available(&host, 1, 4, 4, 8),
    )
    .unwrap();
    admit(
        &mut owner,
        &host,
        "plan/c",
        "place/c",
        &current,
        item_with_available(&host, 4, 12, 12, 4),
    )
    .unwrap();
    assert_eq!(
        admit(
            &mut owner,
            &host,
            "plan/d",
            "place/d",
            &current,
            item_with_available(&host, 6, 12, 12, 6)
        ),
        Err(ResourceAdmissionRefusal::Overcommitted)
    );
    assert_eq!(owner.admissions().len(), 3, "refusal reserves nothing");

    owner
        .release(&PlanId::from("plan/b"), &PlacementId::from("place/b"))
        .unwrap();
    admit(
        &mut owner,
        &host,
        "plan/d",
        "place/d",
        &current,
        item_with_available(&host, 4, 12, 12, 4),
    )
    .unwrap();
}

#[test]
fn prior_observation_does_not_make_a_racing_second_admission_authoritative() {
    let host = host();
    let prior = observation(&host, 8, "sign/prior-fit");
    let mut owner = ResourceAdmissionOwner::new(host.clone());
    let candidate = item(&host, 4, 8, 8);
    admit(
        &mut owner,
        &host,
        "plan/first",
        "place/model",
        &prior,
        candidate.clone(),
    )
    .unwrap();
    assert_eq!(
        admit(
            &mut owner,
            &host,
            "plan/second",
            "place/model",
            &prior,
            candidate
        ),
        Err(ResourceAdmissionRefusal::Overcommitted)
    );
    assert_eq!(
        owner.admissions()[0].observation_sign_ids,
        [SignId::from("sign/prior-fit")]
    );
}

#[test]
fn lane_assignment_is_transient_bounded_and_reassignable_without_plan_mutation() {
    let host = host();
    let current = observation(&host, 16, "sign/current");
    let mut owner = ResourceAdmissionOwner::new(host.clone());
    let plan = PlanId::from("plan/stable");
    let placement = PlacementId::from("place/model");
    owner
        .admit(
            ResourceAdmissionRequest {
                plan_id: plan.clone(),
                placement_id: placement.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                items: vec![item(&host, 2, 4, 8)],
            },
            core::slice::from_ref(&current),
        )
        .unwrap();
    let play = ActivePlayId::from("play/1");
    let first = [
        BaseExecutionLaneId::from("lane/0"),
        BaseExecutionLaneId::from("lane/1"),
    ];
    owner
        .assign_compute_lanes(&plan, &placement, play.clone(), &first)
        .unwrap();
    let reassigned = [
        BaseExecutionLaneId::from("lane/2"),
        BaseExecutionLaneId::from("lane/3"),
    ];
    owner
        .assign_compute_lanes(&plan, &placement, play.clone(), &reassigned)
        .unwrap();
    assert_eq!(owner.assignments()[0].plan_id, plan);
    assert_eq!(owner.assignments()[0].lanes[0].base_lane_id, reassigned[0]);

    let excessive = (0..5)
        .map(|index| BaseExecutionLaneId::from(format!("lane/{index}")))
        .collect::<Vec<_>>();
    assert_eq!(
        owner.assign_compute_lanes(&plan, &placement, play, &excessive),
        Err(ResourceAdmissionRefusal::TooManyLanes)
    );
    assert_eq!(
        owner.assign_compute_lanes(
            &PlanId::from("plan/foreign"),
            &placement,
            ActivePlayId::from("play/foreign"),
            &first,
        ),
        Err(ResourceAdmissionRefusal::ForeignPlayOrPlacement)
    );
    owner.release(&plan, &placement).unwrap();
    assert!(owner.assignments().is_empty());
}

#[test]
fn stale_boot_generation_and_provider_health_refuse_before_mutation() {
    let host = host();
    let mut owner = ResourceAdmissionOwner::new(host.clone());
    let mut stale = observation(&host, 16, "sign/stale");
    stale.offer_generation = OfferGeneration(6);
    assert_eq!(
        admit(
            &mut owner,
            &host,
            "plan/stale",
            "place/model",
            &stale,
            item(&host, 1, 1, 1)
        ),
        Err(ResourceAdmissionRefusal::StaleObservation)
    );
    let mut lost = observation(&host, 0, "sign/provider-lost");
    lost.health = ResourceHealth::Unavailable;
    assert_eq!(
        admit(
            &mut owner,
            &host,
            "plan/lost",
            "place/model",
            &lost,
            item(&host, 1, 1, 1)
        ),
        Err(ResourceAdmissionRefusal::Unavailable)
    );
    assert!(owner.admissions().is_empty());
}

#[test]
fn every_terminal_path_releases_the_exact_admission_and_assignment() {
    let host = host();
    let current = observation(&host, 16, "sign/terminal-current");
    for (index, cause) in [
        ResourceReleaseCause::Completed,
        ResourceReleaseCause::Cancelled,
        ResourceReleaseCause::FailedStart,
        ResourceReleaseCause::Aborted,
    ]
    .into_iter()
    .enumerate()
    {
        let plan = PlanId::from(format!("plan/terminal/{index}"));
        let placement = PlacementId::from("place/model");
        let mut owner = ResourceAdmissionOwner::new(host.clone());
        owner
            .admit(
                ResourceAdmissionRequest {
                    plan_id: plan.clone(),
                    placement_id: placement.clone(),
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    offer_generation: host.offer_generation,
                    items: vec![item(&host, 1, 1, 1)],
                },
                core::slice::from_ref(&current),
            )
            .unwrap();
        owner
            .assign_compute_lanes(
                &plan,
                &placement,
                ActivePlayId::from(format!("play/{index}")),
                &[BaseExecutionLaneId::from("lane/0")],
            )
            .unwrap();
        let released = owner.release_for(&plan, &placement, cause).unwrap();
        assert_eq!(released.admission.plan_id, plan);
        assert_eq!(released.cause, cause);
        assert!(owner.admissions().is_empty());
        assert!(owner.assignments().is_empty());
    }
}
