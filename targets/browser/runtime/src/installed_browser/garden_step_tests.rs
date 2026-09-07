use super::*;
use conduit_core::{OfferGeneration, Scalar};

fn placement() -> PlannedGear {
    placement_for(offer())
}

fn placement_for(offered: CapabilityOffer) -> PlannedGear {
    PlannedGear {
        placement_id: "garden-step-placement".into(),
        gear_id: "garden-step".into(),
        kind_id: offered.kind_id,
        kind_contract_revision: offered.kind_contract_revision,
        execution_profile_id: offered.implementation.execution_profile_id,
        configuration: Vec::new(),
        host_id: "browser/garden".into(),
        boot_id: "browser-boot/garden".into(),
        offer_generation: OfferGeneration(1),
        capability_id: offered.capability_id,
        implementation_id: offered.implementation.implementation_id,
        artifact_id: offered.implementation.artifact_id,
        realization_characteristics: Vec::new(),
        limits: offered.limits,
        inputs: offered.inputs,
        outputs: offered.outputs,
        host_operations: offered.host_operations,
        resources: Vec::new(),
        authority: Vec::new(),
        pool_references: Vec::new(),
    }
}

#[test]
fn exact_state_and_clock_execute_the_shared_reducer() {
    let mut prepared = PreparedGardenStep::for_placement(&placement())
        .unwrap()
        .unwrap();
    let prior = conduit_semantic_catalog::GardenState {
        vitality: Scalar::from_raw_microunits(400_000),
        activity: Scalar::ZERO,
        step: 0,
    };
    let clock = conduit_semantic_catalog::GardenClockObservation {
        phase: Scalar::from_raw_microunits(800_000),
    };
    let prior = conduit_semantic_catalog::garden_state_value(prior)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let clock = conduit_semantic_catalog::garden_clock_observation_value(clock)
        .unwrap()
        .canonical_bytes()
        .unwrap();

    assert_eq!(prepared.execute(OPERATIONS[0], &prior), Ok(None));
    let output = prepared.execute(OPERATIONS[1], &clock).unwrap().unwrap();
    let next = conduit_semantic_catalog::decode_garden_state(&output).unwrap();
    assert_eq!(next.vitality.raw_microunits(), 500_000);
    assert_eq!(next.activity.raw_microunits(), 400_000);
    assert_eq!(next.step, 1);
}

#[test]
fn wrong_order_and_wrong_typed_input_refuse() {
    let mut prepared = PreparedGardenStep::for_placement(&placement())
        .unwrap()
        .unwrap();
    let clock = conduit_semantic_catalog::garden_clock_observation_value(
        conduit_semantic_catalog::GardenClockObservation {
            phase: Scalar::from_raw_microunits(800_000),
        },
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();

    assert_eq!(prepared.execute(OPERATIONS[1], &clock), Err(failure(3)));
    assert_eq!(prepared.execute(OPERATIONS[0], &clock), Err(failure(2)));
}

#[test]
fn contact_variant_requires_and_applies_the_explicit_third_source() {
    let mut prepared = PreparedGardenStep::for_placement(&placement_for(contact_offer()))
        .unwrap()
        .unwrap();
    let prior =
        conduit_semantic_catalog::garden_state_value(conduit_semantic_catalog::GardenState {
            vitality: Scalar::from_raw_microunits(400_000),
            activity: Scalar::ZERO,
            step: 0,
        })
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let clock = conduit_semantic_catalog::garden_clock_observation_value(
        conduit_semantic_catalog::GardenClockObservation {
            phase: Scalar::from_raw_microunits(800_000),
        },
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let contact = conduit_semantic_catalog::garden_contact_observation_value(
        conduit_semantic_catalog::GardenContactObservation {
            intensity: Scalar::from_raw_microunits(800_000),
        },
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();

    assert_eq!(prepared.execute(CONTACT_OPERATIONS[0], &prior), Ok(None));
    assert_eq!(prepared.execute(CONTACT_OPERATIONS[1], &clock), Ok(None));
    let output = prepared
        .execute(CONTACT_OPERATIONS[2], &contact)
        .unwrap()
        .unwrap();
    let next = conduit_semantic_catalog::decode_garden_state(&output).unwrap();
    assert_eq!(next.vitality.raw_microunits(), 600_000);
    assert_eq!(next.activity.raw_microunits(), 400_000);
    assert_eq!(next.step, 1);
}
