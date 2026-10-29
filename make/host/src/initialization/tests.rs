use super::*;
use conduit_core::KindConfigurationRule;
fn contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![
            KindConfigurationField {
                key: "endpoint".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::DataLabel { maximum: 64 },
            },
            KindConfigurationField {
                key: "duration_ms".into(),
                default_value: ConfigurationValue::U64(1),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: 6000,
                },
            },
        ],
    }
}
fn input(value: serde_json::Value) -> BaseInitialization {
    serde_json::from_value(value).unwrap()
}
#[test]
fn explicit_values_use_existing_finite_configuration_rules() {
    let values = input(serde_json::json!({"endpoint":"CMTECK", "duration_ms":3000}))
        .checked_values(&contract())
        .unwrap();
    assert_eq!(values["duration_ms"], ConfigurationValue::U64(3000));
    assert_eq!(
        values["endpoint"],
        ConfigurationValue::Text("CMTECK".into())
    );
}
#[test]
fn missing_unknown_wrong_type_and_unbounded_values_refuse() {
    for value in [
        serde_json::json!({"endpoint":"CMTECK"}),
        serde_json::json!({"endpoint":"CMTECK", "duration_ms":1,"extra":true}),
        serde_json::json!({"endpoint":"", "duration_ms":1}),
        serde_json::json!({"endpoint":"CMTECK", "duration_ms":0}),
        serde_json::json!({"endpoint":"CMTECK", "duration_ms":6001}),
        serde_json::json!({"endpoint":"CMTECK", "duration_ms":"3s"}),
        serde_json::json!({"endpoint":"CMTECK", "duration_ms":1.5}),
        serde_json::json!({"endpoint":null, "duration_ms":1}),
        serde_json::json!({"endpoint":["CMTECK"], "duration_ms":1}),
    ] {
        assert!(input(value).checked_values(&contract()).is_err());
    }
}
#[test]
fn duplicate_contract_fields_and_oversized_inputs_refuse() {
    let mut c = contract();
    c.fields.push(c.fields[0].clone());
    assert!(
        input(serde_json::json!({"endpoint":"CMTECK", "duration_ms":1}))
            .checked_values(&c)
            .is_err()
    );
    assert!(input(
        serde_json::json!({"endpoint":"x".repeat(MAXIMUM_SOURCE_BYTES), "duration_ms":1})
    )
    .checked_values(&contract())
    .is_err());
}
#[test]
fn unsupported_unbounded_contracts_and_source_injection_refuse() {
    let mut c = contract();
    c.fields[0].rule = KindConfigurationRule::Any;
    assert!(c.validate().is_err());
    let mut value = input(serde_json::json!({"endpoint":"CMTECK", "duration_ms":1}));
    value
        .values
        .insert("bad name\nbase".into(), serde_json::json!("x"));
    assert!(value.validate_source().is_err());
}
#[test]
fn package_cannot_replace_another_implementations_initialization_contract() {
    let packages = crate::test_packages::test_package_set();
    let mut contributions = packages.contributions().to_vec();
    let contribution = contributions.first_mut().unwrap();
    let catalog = match contribution {
        crate::MakeContribution::Anchor(a) => &mut a.catalog,
        crate::MakeContribution::Extension(e) => &mut e.catalog,
    };
    catalog
        .initialization_contracts
        .insert("fixture/unowned-implementation@1".into(), contract());
    let diagnostics = crate::MakePackageSet::from_contributions(contributions).unwrap_err();
    assert!(diagnostics.iter().any(|d| matches!(
        d,
        crate::PackageCompositionDiagnostic::InitializationContract { .. }
    )));
}
