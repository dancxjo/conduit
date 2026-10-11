use super::*;
use alloc::vec;
use conduit_core::*;
fn gear(text: &str) -> PlannedGear {
    let expected = offer();
    conduit_core::planned_gear_from_parts! {
        placement_id: "literal-placement".into(),
        gear_id: "literal".into(),
        kind_id: expected.kind_id,
        kind_contract_revision: expected.kind_contract_revision,
        execution_profile_id: expected.implementation.execution_profile_id,
        configuration: vec![ConfigurationEntry {
            key: "hex".into(),
            value: ConfigurationValue::Text(text.into()),
        }],
        host_id: "literal-host".into(),
        boot_id: "literal-boot".into(),
        offer_generation: OfferGeneration(1),
        capability_id: expected.capability_id,
        implementation_id: expected.implementation.implementation_id,
        artifact_id: expected.implementation.artifact_id,
        base: None,
        realization_characteristics: vec![],
        limits: expected.limits,
        inputs: expected.inputs,
        outputs: expected.outputs,
        semantic_contract: expected.semantic_contract,
        terminal_transductions: vec![],
        host_calls: vec![],
        resources: vec![],
        authority: vec![],
        pool_references: vec![],
    }
}
#[test]
fn one_prepared_value_has_no_host_calls_and_exact_authored_octets() {
    let factory = BytesLiteralOperationFactory::default();
    let gear = gear("00ff80");
    let budget = factory.budget(&gear).unwrap();
    assert_eq!(budget.value_items, 1);
    assert_eq!(budget.host_requests, 0);
    assert_eq!(budget.value_bytes, 3);
    let mut values =
        HostedValueStore::new(1, budget.maximum_value_bytes, budget.value_bytes).unwrap();
    let _back = factory.prepare(&gear, &mut values).unwrap();
    assert_eq!(values.used_items(), 1);
    assert_eq!(values.used_bytes(), budget.value_bytes);
    assert!(factory.prepare(&gear, &mut values).is_err());
}
#[test]
fn empty_literal_keeps_a_real_finite_value_slot() {
    let factory = BytesLiteralOperationFactory::default();
    let gear = gear("");
    let budget = factory.budget(&gear).unwrap();
    assert_eq!(budget.value_bytes, 1);
    let mut values = HostedValueStore::new(1, 1, 1).unwrap();
    factory.prepare(&gear, &mut values).unwrap();
    assert_eq!(values.used_items(), 1);
    assert_eq!(values.used_bytes(), 0);
}
#[test]
fn changed_front_and_selected_implementation_are_refused_before_storage() {
    let factory = BytesLiteralOperationFactory::default();
    let mut changed = gear("00");
    changed.outputs[0].value_kind = kind_id("value/u64");
    assert!(factory.budget(&changed).is_err());
    changed = gear("00");
    changed.artifact_id = "another-artifact".into();
    assert!(factory.budget(&changed).is_err());
    changed = gear("00");
    changed.limits.max_queue_bytes += 1;
    assert!(factory.budget(&changed).is_err());
    let mut values = HostedValueStore::new(1, 1, 1).unwrap();
    assert!(factory.prepare(&changed, &mut values).is_err());
    assert_eq!(values.used_items(), 0);
}
#[test]
fn missing_duplicate_unknown_and_oversize_configuration_refuse() {
    let factory = BytesLiteralOperationFactory::default();
    let mut changed = gear("00");
    changed.configuration.clear();
    assert!(factory.budget(&changed).is_err());
    changed = gear("00");
    changed.configuration.push(changed.configuration[0].clone());
    assert!(factory.budget(&changed).is_err());
    changed = gear("00");
    changed.configuration[0].key = "other".into();
    assert!(factory.budget(&changed).is_err());
    assert!(
        factory
            .budget(&gear(&"00".repeat(
                conduit_data::bytes_literal::MAXIMUM_LITERAL_BYTES as usize + 1
            )))
            .is_err()
    );
}
#[test]
fn foreign_effect_requirement_cannot_turn_literal_into_a_host_call() {
    let factory = BytesLiteralOperationFactory::default();
    let mut changed = gear("00");
    changed.host_calls = crate::functional_offers::text_upper_offer().host_calls;
    assert!(factory.budget(&changed).is_err());
}

#[test]
fn odd_nonhex_and_forged_effects_refuse_before_value_storage() {
    let factory = BytesLiteralOperationFactory::default();
    for hex in ["0", "zz", "00 1", "é"] {
        let mut values = HostedValueStore::new(1, 2048, 2048).unwrap();
        assert!(factory.prepare(&gear(hex), &mut values).is_err());
        assert_eq!(values.used_items(), 0);
    }
    let mut forged = gear("80");
    forged.outputs[0].temporal = PortTemporal::Flow { closes: true };
    assert!(factory.budget(&forged).is_err());
}
