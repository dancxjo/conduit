use conduit_body::{
    Body, BodyResourceAllowance, BodyResourceEnvelope, BodyResourceReservationError,
    BodyResourceReservationLedger, PartId, MAX_BODY_RESOURCE_RESERVATIONS,
};
use conduit_core::{
    resource_offer, resource_requirement, BootId, CheckedFormId, HostAdvertisement, HostId,
    HostProfileId, OfferGeneration, PlanId, ResourceBinding, ResourceHealth, ResourceObservation,
    SignId, SourceDocumentId, PROTOCOL_VERSION,
};

fn fixture(
    generation: u64,
    offered: u32,
) -> (HostAdvertisement, BodyResourceEnvelope, ResourceObservation) {
    let body = Body::born(
        SourceDocumentId::from("reservation-source"),
        CheckedFormId::from("reservation-form"),
        1,
        SignId::from("body-born"),
    )
    .unwrap();
    let part = PartId::bind(&body.body_id, "workstation", 1).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("workstation"),
        boot_id: BootId::from("workstation/boot"),
        offer_generation: OfferGeneration(generation),
        profile: HostProfileId::from("workstation"),
        resources: vec![resource_offer(
            "execution",
            "test/execution",
            offered.max(16),
        )],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let envelope = BodyResourceEnvelope::new(
        body.body_id,
        part,
        &host,
        vec![BodyResourceAllowance {
            pool_id: "execution".into(),
            class_id: "test/execution".into(),
            maximum_units: offered,
        }],
    )
    .unwrap();
    let observation = ResourceObservation {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        pool_id: "execution".into(),
        class_id: "test/execution".into(),
        health: ResourceHealth::Ready,
        unreserved_units: offered.max(16),
        utilized_units: 0,
        sign_id: SignId::from("resource-now"),
    };
    (host, envelope, observation)
}

fn binding(units: u32) -> ResourceBinding {
    ResourceBinding {
        pool_id: "execution".into(),
        class_id: "test/execution".into(),
        units,
        protected: None,
        compute: None,
    }
}

#[test]
fn overlapping_plans_cannot_cumulatively_exceed_body_offer() {
    let (host, envelope, observation) = fixture(7, 6);
    let requirement = resource_requirement("test/execution", 4);
    let four = binding(4);
    let mut ledger = BodyResourceReservationLedger::new(&envelope);

    ledger
        .reserve(
            PlanId::from("plan-a"),
            &envelope,
            &host,
            core::slice::from_ref(&observation),
            &[(&requirement, &four)],
        )
        .unwrap();
    assert_eq!(ledger.reserved_units(&four), 4);
    assert_eq!(
        ledger.reserve(
            PlanId::from("plan-b"),
            &envelope,
            &host,
            core::slice::from_ref(&observation),
            &[(&requirement, &four)],
        ),
        Err(BodyResourceReservationError::Envelope(
            conduit_body::BodyResourceEnvelopeError::ReservationExceedsAllowance
        ))
    );
    assert_eq!(ledger.reservations().len(), 1, "refusal is atomic");
}

#[test]
fn one_plan_aggregates_multiple_placements_on_the_same_pool_atomically() {
    let (host, envelope, observation) = fixture(7, 6);
    let requirement = resource_requirement("test/execution", 2);
    let two_a = binding(2);
    let two_b = binding(2);
    let mut ledger = BodyResourceReservationLedger::new(&envelope);
    ledger
        .reserve(
            PlanId::from("two-placement-plan"),
            &envelope,
            &host,
            core::slice::from_ref(&observation),
            &[(&requirement, &two_a), (&requirement, &two_b)],
        )
        .unwrap();
    assert_eq!(ledger.reserved_units(&two_a), 4);

    let four = binding(4);
    let four_requirement = resource_requirement("test/execution", 4);
    assert!(matches!(
        ledger.reserve(
            PlanId::from("overlapping-plan"),
            &envelope,
            &host,
            core::slice::from_ref(&observation),
            &[(&four_requirement, &four)],
        ),
        Err(BodyResourceReservationError::Envelope(
            conduit_body::BodyResourceEnvelopeError::ReservationExceedsAllowance
        ))
    ));
    assert_eq!(ledger.reservations().len(), 1);
}

#[test]
fn release_restores_quota_and_unknown_or_duplicate_plan_refuses() {
    let (host, envelope, observation) = fixture(7, 4);
    let requirement = resource_requirement("test/execution", 4);
    let four = binding(4);
    let plan = PlanId::from("plan-a");
    let mut ledger = BodyResourceReservationLedger::new(&envelope);
    let reserve = |ledger: &mut BodyResourceReservationLedger| {
        ledger.reserve(
            plan.clone(),
            &envelope,
            &host,
            core::slice::from_ref(&observation),
            &[(&requirement, &four)],
        )
    };
    reserve(&mut ledger).unwrap();
    assert_eq!(
        reserve(&mut ledger),
        Err(BodyResourceReservationError::DuplicatePlan)
    );
    let released = ledger.release(&plan).unwrap();
    assert_eq!(released.plan_id(), &plan);
    assert!(ledger.reservations().is_empty());
    assert_eq!(
        ledger.release(&plan),
        Err(BodyResourceReservationError::UnknownPlan)
    );
    reserve(&mut ledger).unwrap();
}

#[test]
fn changed_offer_creates_a_new_fence_without_mutating_old_plan_accounting() {
    let (old_host, old_envelope, old_observation) = fixture(7, 6);
    let requirement = resource_requirement("test/execution", 4);
    let four = binding(4);
    let old_plan = PlanId::from("immutable-old-plan");
    let mut ledger = BodyResourceReservationLedger::new(&old_envelope);
    ledger
        .reserve(
            old_plan.clone(),
            &old_envelope,
            &old_host,
            core::slice::from_ref(&old_observation),
            &[(&requirement, &four)],
        )
        .unwrap();

    let (new_host, new_envelope, new_observation) = fixture(8, 2);
    assert_ne!(old_envelope.envelope_id(), new_envelope.envelope_id());
    assert_eq!(
        ledger.reserve(
            PlanId::from("new-plan"),
            &new_envelope,
            &new_host,
            core::slice::from_ref(&new_observation),
            &[(&requirement, &four)],
        ),
        Err(BodyResourceReservationError::EnvelopeMismatch)
    );
    assert_eq!(ledger.reservations()[0].plan_id(), &old_plan);
    assert_eq!(
        ledger.reservations()[0].bindings(),
        core::slice::from_ref(&four)
    );
}

#[test]
fn reservation_count_is_finite_and_inspectable() {
    let (host, envelope, observation) = fixture(7, 64);
    let requirement = resource_requirement("test/execution", 1);
    let one = binding(1);
    let mut ledger = BodyResourceReservationLedger::new(&envelope);
    for index in 0..MAX_BODY_RESOURCE_RESERVATIONS {
        ledger
            .reserve(
                PlanId::from(format!("plan-{index}")),
                &envelope,
                &host,
                core::slice::from_ref(&observation),
                &[(&requirement, &one)],
            )
            .unwrap();
    }
    assert_eq!(ledger.reservations().len(), MAX_BODY_RESOURCE_RESERVATIONS);
    assert_eq!(
        ledger.reserve(
            PlanId::from("one-too-many"),
            &envelope,
            &host,
            core::slice::from_ref(&observation),
            &[(&requirement, &one)],
        ),
        Err(BodyResourceReservationError::CapacityExceeded)
    );
}
