//! Configuration encoding only; these fixtures establish no Plan or authority.
use super::*;
use conduit_core as c;
fn fixture(publish: bool) -> (PlanFragment, String) {
    let kind = conduit_semantic_catalog::resource_snapshot_semantic_contract(publish);
    let reference = c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: if publish {
            kind.inputs[0].value_kind.clone()
        } else {
            kind.outputs[0].value_kind.clone()
        },
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: 2,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    let mut text = String::new();
    for byte in reference.encode().unwrap() {
        use core::fmt::Write;
        write!(&mut text, "{byte:02x}").unwrap();
    }
    let mut fragment = crate::tests::sealed_current_fragment();
    fragment.placements.truncate(1);
    let gear = &mut fragment.placements[0];
    gear.kind_id = kind.kind_id;
    gear.kind_contract_revision = kind.kind_contract_revision;
    gear.inputs = kind.inputs;
    gear.outputs = kind.outputs;
    gear.semantic_contract = KindSemanticContract {
        configuration: kind.configuration,
        laws: kind.semantic_laws,
    };
    gear.configuration = vec![c::ConfigurationEntry {
        key: "reference".into(),
        value: ConfigurationValue::Text(text.clone()),
    }];
    (fragment, text)
}
#[test]
fn snapshot_reference_is_preserved_only_for_the_exact_canonical_kind() {
    for publish in [false, true] {
        let (fragment, text) = fixture(publish);
        let encoded = generate_configuration(&fragment, None).unwrap();
        assert_eq!(encoded.len(), 1);
        assert_eq!(encoded[0].key, "reference");
        assert_eq!(encoded[0].value, GeneratedConfigurationValue::Text(text));
        for change in 0..6 {
            let mut changed = fragment.clone();
            let gear = &mut changed.placements[0];
            match change {
                0 => gear.kind_id = "other/kind".into(),
                1 => gear.kind_contract_revision = "other/revision".into(),
                2 => gear.inputs[0].port_id = "other-port".into(),
                3 => gear.outputs[0].value_kind = "other/type".into(),
                4 => gear.semantic_contract.laws.clear(),
                5 => gear.configuration.push(gear.configuration[0].clone()),
                _ => unreachable!(),
            }
            assert!(
                generate_configuration(&changed, None).is_err(),
                "mutation {change}"
            );
        }
    }
}
#[test]
fn malformed_oversized_and_unrelated_reference_text_refuse() {
    let (mut fragment, text) = fixture(true);
    for bad in [
        String::new(),
        "x".into(),
        "ff".into(),
        format!("{text}00"),
        "00".repeat(513),
        text.to_uppercase(),
    ] {
        fragment.placements[0].configuration[0].value = ConfigurationValue::Text(bad);
        assert!(generate_configuration(&fragment, None).is_err());
    }
    fragment.placements[0].configuration[0].value = ConfigurationValue::Text(text);
    fragment.placements[0].configuration[0].key = "arbitrary".into();
    assert!(generate_configuration(&fragment, None).is_err());
}
