//! Concrete installed adapter and offer proof; not a funded IMAGE input-provider proof.
use super::*;
use crate::installed_std::{
    prepared_face_route::PreparedFaceProjector,
    quantity_observation_route::QuantityObservationProjector,
};
use conduit_core::{ConfigurationEntry, ConfigurationValue};

fn placement() -> PlannedGear {
    let offer = conduit_std_offers::quantity_observation_face_offer().unwrap();
    conduit_core::planned_gear_from_parts! {
        realization_properties: offer.realization_properties,
        semantic_contract: offer.semantic_contract,
        placement_id: "quantity-fixture".into(), gear_id: "quantity-inspector".into(),
        kind_id: offer.kind_id, kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![ConfigurationEntry { key: "label".into(), value: ConfigurationValue::Text("Reported voltage".into()) }],
        host_id: "host/inspector".into(), boot_id: "boot/inspector".into(), offer_generation: conduit_core::OfferGeneration(1),
        capability_id: offer.capability_id, implementation_id: offer.implementation.implementation_id,
        artifact_id: offer.implementation.artifact_id,
        base: None, realization_characteristics: Vec::new(), limits: offer.limits,
        inputs: offer.inputs, outputs: offer.outputs, terminal_transductions: Vec::new(), host_calls: offer.host_calls,
        resources: Vec::new(), authority: Vec::new(), pool_references: Vec::new(),
    }
}

#[test]
fn registered_quantity_factory_accepts_exact_offer_and_refuses_forged_calls_artifact_and_label() {
    let exact = placement();
    assert!(crate::installed_std::catalog::factory(&exact.implementation_id).is_some());
    let scope = budget(&exact).unwrap();
    assert_eq!(scope.host_requests, 2);
    assert_eq!((scope.value_items, scope.value_bytes), (0, 0));
    let mut values = conduit_kernel::HostedValueStore::new(
        1,
        scope.maximum_value_bytes,
        scope.maximum_value_bytes,
    )
    .unwrap();
    let InstalledBack::QuantityObservationFace(back) = prepare(&exact, &mut values).unwrap() else {
        panic!("wrong installed Back");
    };
    assert!(back.retained_backing_bytes().unwrap() > 0);
    for mode in 0..3 {
        let mut forged = exact.clone();
        match mode {
            0 => forged.host_calls[1].maximum_output_bytes = 1,
            1 => forged.artifact_id = "artifact/forged".into(),
            _ => forged.configuration[0].value = ConfigurationValue::Text("".into()),
        }
        assert!(budget(&forged).is_err());
        assert!(prepare(&forged, &mut values).is_err());
    }
}

fn snapshot_bytes() -> Vec<u8> {
    use conduit_data::{
        QuantityObservationOutcome, ResolvedQuantityObservation, ResolvedQuantityObservationBatch,
    };
    use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
    let batch = ResolvedQuantityObservationBatch {
        source_identity: "source/reported-meter".into(),
        source_name: "Reported meter".into(),
        channel_identity: "series/voltage".into(),
        channel_name: "Voltage".into(),
        producer_host: "reported-host".into(),
        producer_boot: "reported-boot".into(),
        producer_plan: "reported-plan".into(),
        producer_play: "reported-play".into(),
        samples_outside_view: 0,
        observations: BoundedSequence::try_from_iter([ResolvedQuantityObservation {
            sequence: 1,
            sampled_at_boot_ms: 7000,
            outcome: QuantityObservationOutcome::Available(Box::new(
                conduit_core::Quantity::parse_plot_literal("12V").unwrap(),
            )),
            gap_before: None,
        }])
        .unwrap(),
    };
    conduit_data::ResolvedQuantityObservationSnapshot {
        batches: BoundedSequence::try_from_iter([batch]).unwrap(),
    }
    .encode()
    .unwrap()
}

#[test]
fn concrete_projector_keeps_reported_provenance_separate_and_rejects_malformed_input() {
    let bytes = snapshot_bytes();
    let original = bytes.clone();
    let basis = conduit_face::FaceContributionBasis {
        checked_plot_id: "checked/inspector".into(),
        plan_id: "plan/inspector".into(),
        active_play_id: "play/inspector".into(),
        required_interaction_context: None,
    };
    let projector = QuantityObservationProjector::prepare().unwrap();
    let fragment = projector
        .project(basis.clone(), &bytes, "Reported voltage")
        .unwrap();
    assert_eq!(bytes, original);
    assert_eq!(fragment.basis, basis);
    assert!(fragment.actions.is_empty());
    assert!(fragment.properties.iter().any(|p| p.name == "producer-play"
        && p.value == conduit_face::FacePropertyValue::Identity("reported-play".into())));
    assert!(
        projector.fragment_backing_bytes(&fragment).unwrap() <= projector.fragment_backing_bound()
    );
    assert!(projector.retained_backing_bytes().unwrap() > 0);
    assert!(projector.projection_workspace_bytes().unwrap() > projector.fragment_backing_bound());
    assert!(projector
        .project(basis, b"not a typed observation", "Reported voltage")
        .is_err());
}
