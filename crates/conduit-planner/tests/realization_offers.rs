use conduit_core::{ArtifactId, CapabilityId, ImplementationId, ImplementationOffer};
use conduit_form::parse;
use conduit_planner::{default_placements, plan, PlacementChoice};
use conduit_signal::{pico_local_advertisement, signal_profile_catalog, PULSE_KIND};

fn pulse_form() -> conduit_form::CheckedForm {
    parse(
        "form 0\n\nrealization {\n    pulse: flow/pulse\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n}\n",
        &signal_profile_catalog(),
    )
    .expect("pulse form checks")
}

#[test]
fn one_host_offers_and_plans_distinct_implementations_of_the_same_face() {
    let form = pulse_form();
    let mut host = pico_local_advertisement();
    let original = host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == PULSE_KIND)
        .expect("pulse realization exists")
        .clone();
    let mut alternate = original.clone();
    alternate.capability_id = CapabilityId::from("pico-w/pulse-alternate");
    alternate.implementation = ImplementationOffer {
        execution_profile_id: original.implementation.execution_profile_id.clone(),
        implementation_id: ImplementationId::from("pico-w/pulse-alternate-v1"),
        artifact_id: ArtifactId::from("conduit-signal/pulse-alternate-artifact-v1"),
    };
    assert_eq!(
        alternate.checked_face(),
        original.checked_face(),
        "realization identity does not alter the semantic face"
    );
    host.capabilities.push(alternate.clone());

    let operation = &form.operations[0];
    let mut placements = default_placements(&form, std::slice::from_ref(&host))
        .expect("equal-face realizations are candidates");
    placements.by_operation.insert(
        operation.operation_id.clone(),
        PlacementChoice {
            host_id: host.host_id.clone(),
            capability_id: alternate.capability_id.clone(),
        },
    );

    let plan = plan(&form, std::slice::from_ref(&host), &placements, &[])
        .expect("the exact alternate realization plans");
    let planned = &plan.fragments[0].placements[0];
    assert_eq!(planned.capability_id, alternate.capability_id);
    assert_eq!(
        planned.execution_profile_id,
        alternate.implementation.execution_profile_id
    );
    assert_eq!(
        planned.implementation_id,
        alternate.implementation.implementation_id
    );
    assert_eq!(planned.artifact_id, alternate.implementation.artifact_id);
}

#[test]
fn implementation_offer_keeps_the_existing_advertisement_wire_shape() {
    let advertisement = pico_local_advertisement();
    let encoded = serde_json::to_value(&advertisement).expect("advertisement serializes");
    let capability = &encoded["capabilities"][0];
    assert!(capability.get("implementation").is_none());
    assert!(capability.get("execution_profile_id").is_some());
    assert!(capability.get("implementation_id").is_some());
    assert!(capability.get("artifact_id").is_some());

    let decoded = serde_json::from_value(encoded).expect("flat advertisement deserializes");
    assert_eq!(advertisement, decoded);
}
