//! Exact cumulative Body admission for the namesake and its unrelated peer Form.

use conduit_body::{
    Body, BodyFormResourceRequests, BodyPlan, BodyResourceAllowance, BodyResourceEnvelope,
    BodyResourceReservationLedger, PartId,
};
use conduit_core::{
    HostAdvertisement, Plan, ResourceBinding, ResourceHealth, ResourceObservation,
    ResourceRequirement, SignId,
};

pub(super) fn admit_combined_resources(
    body: &Body,
    body_plan: &BodyPlan,
    host: &HostAdvertisement,
    secret_knock: &Plan,
    unrelated: &Plan,
) {
    let part = PartId::bind(&body.body_id, "secret-knock-std-host", 1).unwrap();
    let allowances = host
        .resources
        .iter()
        .map(|resource| BodyResourceAllowance {
            pool_id: resource.pool_id.clone(),
            class_id: resource.class_id.clone(),
            maximum_units: resource.capacity_units,
        })
        .collect::<Vec<_>>();
    let envelope = BodyResourceEnvelope::new(body.body_id.clone(), part, host, allowances).unwrap();
    let observations = host
        .resources
        .iter()
        .enumerate()
        .map(|(index, resource)| ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: resource.pool_id.clone(),
            class_id: resource.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: resource.capacity_units,
            utilized_units: 0,
            sign_id: SignId::from(format!("sign/secret-knock-resource-{index}")),
        })
        .collect::<Vec<_>>();

    let secret_demands = exact_demands(secret_knock, host);
    let unrelated_demands = exact_demands(unrelated, host);
    let secret_requests = references(&secret_demands);
    let unrelated_requests = references(&unrelated_demands);
    let partitions = body_plan
        .forms
        .iter()
        .map(|partition| {
            let requests = if partition.plan.plan_id == secret_knock.plan_id {
                secret_requests.as_slice()
            } else if partition.plan.plan_id == unrelated.plan_id {
                unrelated_requests.as_slice()
            } else {
                panic!("Body plan contains an unexpected constituent Plan")
            };
            BodyFormResourceRequests {
                form: &partition.form,
                requests,
            }
        })
        .collect::<Vec<_>>();
    let expected_bindings = secret_requests.len() + unrelated_requests.len();
    assert!(
        expected_bindings >= 5,
        "clock, deadline, storage, and presentation demand must remain explicit"
    );

    let mut ledger = BodyResourceReservationLedger::new(&envelope);
    ledger
        .reserve_body_plan(body_plan, &envelope, host, &observations, &partitions)
        .expect("all Form demand is admitted atomically before Body Play start");
    assert_eq!(ledger.reservations().len(), 1);
    assert_eq!(ledger.reservations()[0].plan_id(), &body_plan.plan_id);
    assert_eq!(ledger.reservations()[0].bindings().len(), expected_bindings);
    for class in [
        conduit_core::TIMER_RESOURCE_CLASS,
        conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS,
        conduit_std_offers::TEMPLATE_STORAGE_RESOURCE_CLASS,
        conduit_core::PRESENTATION_RESOURCE_CLASS,
    ] {
        assert!(ledger.reservations()[0]
            .bindings()
            .iter()
            .any(|binding| binding.class_id.as_str() == class));
    }

    for plan in [secret_knock, unrelated] {
        assert!(plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .all(|cord| cord.item_capacity > 0 && cord.byte_capacity > 0));
    }
}

fn exact_demands(
    plan: &Plan,
    host: &HostAdvertisement,
) -> Vec<(ResourceRequirement, ResourceBinding)> {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .flat_map(|placement| {
            let offer = host
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == placement.capability_id)
                .expect("planned capability remains in exact Host advertisement");
            placement.resources.iter().map(|binding| {
                let requirement = offer
                    .resource_requirements
                    .iter()
                    .find(|requirement| {
                        requirement.class_id == binding.class_id
                            && requirement.units == binding.units
                    })
                    .expect("selected binding retains its semantic requirement")
                    .clone();
                (requirement, binding.clone())
            })
        })
        .collect()
}

fn references(
    demands: &[(ResourceRequirement, ResourceBinding)],
) -> Vec<(&ResourceRequirement, &ResourceBinding)> {
    demands
        .iter()
        .map(|(requirement, binding)| (requirement, binding))
        .collect()
}
