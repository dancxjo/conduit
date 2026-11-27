//! Concrete installed adapter and offer proof; not a funded IMAGE input-provider proof.
use super::*;
use conduit_core::{ConfigurationEntry, ConfigurationValue};

fn placement() -> PlannedGear {
    let offer = conduit_std_offers::reported_generation_face_offer().unwrap();
    conduit_core::planned_gear_from_parts! {
        realization_properties: offer.realization_properties,
        semantic_contract: offer.semantic_contract,
        placement_id: "recording-fixture".into(), gear_id: "recording-inspector".into(),
        kind_id: offer.kind_id, kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![ConfigurationEntry { key: "label".into(), value: ConfigurationValue::Text("Reported recording".into()) }],
        host_id: "host/inspector".into(), boot_id: "boot/inspector".into(), offer_generation: conduit_core::OfferGeneration(1),
        capability_id: offer.capability_id, implementation_id: offer.implementation.implementation_id,
        artifact_id: offer.implementation.artifact_id,
        base: None, realization_characteristics: Vec::new(), limits: offer.limits,
        inputs: offer.inputs, outputs: offer.outputs, terminal_transductions: Vec::new(), host_calls: offer.host_calls,
        resources: Vec::new(), authority: Vec::new(), pool_references: Vec::new(),
    }
}

#[test]
fn registered_reported_generation_factory_accepts_exact_offer_and_refuses_forged_calls_artifact_and_label(
) {
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
    let InstalledBack::ReportedGenerationFace(back) = prepare(&exact, &mut values).unwrap() else {
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
