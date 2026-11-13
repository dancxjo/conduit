use super::*;
fn fixture(name: &str) -> (StructuredInfoType, CheckedValueContract) {
    let schema = StructuredInfoType::record(
        kind_id(name),
        vec![StructuredFieldType::new(
            "setting",
            StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    let contract =
        CheckedValueContract::new(schema.profile().unwrap().value_kind().clone(), 512, vec![])
            .unwrap();
    (schema, contract)
}
#[test]
fn exact_types_have_distinct_producers_without_embedded_host_material() {
    let (first, contract) = fixture("fixture/first-settings");
    let (second, other) = fixture("fixture/second-settings");
    let kind = installed_value_once_semantic_contract(&contract, &first).unwrap();
    let second = installed_value_once_semantic_contract(&other, &second).unwrap();
    assert_ne!(kind.kind_id, second.kind_id);
    assert_eq!(
        kind.kind_id,
        typed_kind_instance_id(
            INSTALLED_VALUE_ONCE_KIND,
            core::iter::once((&first, &contract))
        )
        .unwrap()
    );
    assert!(kind.inputs.is_empty());
    assert_eq!(kind.outputs.len(), 1);
    assert_eq!(
        kind.outputs[0].temporal,
        PortTemporal::Flow { closes: true }
    );
    assert_eq!(kind.outputs[0].port_id, port_id("value"));
    assert_eq!(kind.configuration.len(), 1);
    assert_eq!(kind.configuration[0].key, "key");
    assert_eq!(
        kind.configuration[0].default_value,
        ConfigurationValue::Text("".into())
    );
    assert!(!kind.startup_parameters[0].has_default);
    assert!(installed_value_once_key(&[]).is_err());
    assert!(installed_value_once_key(&[ConfigurationEntry {
        key: "key".into(),
        value: kind.configuration[0].default_value.clone()
    }])
    .is_err());
    assert_eq!(
        kind.semantic_contract()
            .value_contracts()
            .iter()
            .find(|v| v.location == FrontValueLocation::Output(port_id("value")))
            .unwrap()
            .contract,
        contract
    );
}
#[test]
fn mismatched_or_unbounded_material_contracts_refuse() {
    let (schema, mut contract) = fixture("fixture/settings");
    contract.value_kind = kind_id("value/u8");
    assert!(installed_value_once_semantic_contract(&contract, &schema).is_err());
    contract.value_kind = schema.profile().unwrap().value_kind().clone();
    contract.maximum_bytes = 0;
    assert!(installed_value_once_semantic_contract(&contract, &schema).is_err());
    contract.maximum_bytes = MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 + 1;
    assert!(installed_value_once_semantic_contract(&contract, &schema).is_err());
}
#[test]
fn key_selection_is_exact_bounded_and_has_no_fallback() {
    let key = |text: &str| ConfigurationEntry {
        key: "key".into(),
        value: ConfigurationValue::Text(text.into()),
    };
    assert_eq!(
        installed_value_once_key(&[key("sensor-settings")]),
        Ok("sensor-settings")
    );
    assert!(installed_value_once_key(&[key("")]).is_err());
    assert!(installed_value_once_key(&[key(
        &"x".repeat(INSTALLED_VALUE_KEY_MAXIMUM_BYTES as usize + 1)
    )])
    .is_err());
    assert!(installed_value_once_key(&[key("a"), key("a")]).is_err());
    assert!(installed_value_once_key(&[ConfigurationEntry {
        key: "address".into(),
        value: ConfigurationValue::U64(118)
    }])
    .is_err());
}
#[cfg(feature = "plot-catalog")]
#[test]
fn catalog_installs_required_logical_key_and_exact_fore() {
    let (schema, contract) = fixture("fixture/settings");
    let kind = installed_value_once_semantic_contract(&contract, &schema).unwrap();
    let (mut startup, mut profiles) = (
        conduit_plot::StartupCatalog::new(),
        conduit_plot::ProfileCatalog::new(),
    );
    install_installed_value_once_kind(&contract, &schema, &mut startup, &mut profiles).unwrap();
    assert_eq!(profiles.get(&kind.kind_id).unwrap().inputs, kind.inputs);
    assert_eq!(profiles.get(&kind.kind_id).unwrap().outputs, kind.outputs);
}
