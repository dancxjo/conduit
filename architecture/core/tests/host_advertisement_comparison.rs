use conduit_core::*;
use serde::de::DeserializeSeed;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;
fn host() -> HostAdvertisement {
    HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/fixture".into(),
        boot_id: "boot/fixture".into(),
        offer_generation: OfferGeneration(1),
        profile: "profile/fixture".into(),
        bases: vec![BaseProviderAdvertisement {
            base_id: "base/fixture".into(),
            provider_instance_id: "provider/fixture".into(),
            provider_generation: 1,
            implementation_id: "implementation/fixture".into(),
            mechanism_family: "mechanism/fixture".into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capability_ids: vec!["capability/first".into(), "capability/second".into()],
            resource_pool_ids: vec!["pool/fixture".into()],
        }],
        resources: vec![ResourceOffer {
            pool_id: "pool/fixture".into(),
            class_id: "class/fixture".into(),
            capacity_units: 16,
            compute: None,
            content: None,
        }],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
fn compare(expected: &HostAdvertisement, bytes: &[u8]) -> bool {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    HostAdvertisementEqualitySeed::new(expected)
        .unwrap()
        .deserialize(&mut decoder)
        .is_ok()
        && decoder.end().is_ok()
}
fn parity(expected: &HostAdvertisement, value: &serde_json::Value) {
    let bytes = serde_json::to_vec(value).unwrap();
    let ordinary =
        serde_json::from_slice::<HostAdvertisement>(&bytes).is_ok_and(|host| host == *expected);
    assert_eq!(compare(expected, &bytes), ordinary, "{value}");
}
#[test]
fn plain_host_comparison_preserves_derived_fields_defaults_order_and_unknowns() {
    let expected = host();
    let original = serde_json::to_value(&expected).unwrap();
    parity(&expected, &original); // Value key order differs from struct order.
    for field in ["declared_name", "planner_capabilities"] {
        let mut value = original.clone();
        value.as_object_mut().unwrap().remove(field);
        parity(&expected, &value);
    }
    for field in [
        "protocol_version",
        "host_id",
        "boot_id",
        "offer_generation",
        "profile",
        "bases",
        "resources",
        "capabilities",
    ] {
        let mut value = original.clone();
        value.as_object_mut().unwrap().remove(field);
        parity(&expected, &value);
    }
    for path in [0, 1, 2] {
        let mut value = original.clone();
        let object = match path {
            0 => value.as_object_mut().unwrap(),
            1 => value["bases"][0].as_object_mut().unwrap(),
            _ => value["resources"][0].as_object_mut().unwrap(),
        };
        object.insert(
            "future_field".into(),
            serde_json::json!({"nested":[1,true,null,"text"]}),
        );
        parity(&expected, &value);
    }
    for case in 0..8 {
        let mut value = original.clone();
        match case {
            0 => value["host_id"] = "foreign/host".into(),
            1 => value["bases"][0]["capability_ids"]
                .as_array_mut()
                .unwrap()
                .reverse(),
            2 => value["bases"][0]["lifecycle"] = "Lost".into(),
            3 => value["resources"][0]["capacity_units"] = 17.into(),
            4 => {
                value["resources"][0]["compute"] = serde_json::json!({"service_guarantee":"Shared","architecture_base_id":"base/foreign","architecture_base_kind":"HostedOs","topology_groups":[]})
            }
            5 => value["resources"][0]["content"] = serde_json::json!({"foreign":"content"}),
            6 => value["capabilities"] = serde_json::json!([{}]),
            _ => value["planner_capabilities"] = serde_json::json!([{}]),
        }
        parity(&expected, &value);
    }
    let mut empty = expected.clone();
    empty.bases.clear();
    let mut value = serde_json::to_value(&empty).unwrap();
    value.as_object_mut().unwrap().remove("bases");
    parity(&empty, &value);
    for text in ["quote\"", "slash\\", "newline\n", "温度"] {
        let mut escaped = expected.clone();
        escaped.profile = text.into();
        parity(&escaped, &serde_json::to_value(&escaped).unwrap());
    }
    let mut named = expected.clone();
    named.declared_name = Some(HostDeclaredName::new("Yellow 温度").unwrap());
    parity(&named, &serde_json::to_value(&named).unwrap());
    let mut value = serde_json::to_value(&named).unwrap();
    value.as_object_mut().unwrap().remove("declared_name");
    parity(&named, &value);
}
#[test]
fn matching_and_contrary_known_fields_require_no_decoded_owners() {
    let expected = host();
    let mut cases = vec![serde_json::to_value(&expected).unwrap()];
    let mut huge_text = cases[0].clone();
    huge_text["host_id"] = "x".repeat(18000).into();
    cases.push(huge_text);
    let mut huge_sequence = cases[0].clone();
    huge_sequence["capabilities"] = serde_json::json!([{"huge":"x".repeat(18000)}]);
    cases.push(huge_sequence);
    let mut huge_option = cases[0].clone();
    huge_option["resources"][0]["compute"] = serde_json::json!({"huge":"x".repeat(18000)});
    cases.push(huge_option);
    for (index, value) in cases.iter().enumerate() {
        let bytes = serde_json::to_vec(value).unwrap();
        let (matched, cost) = probe::observe(|| compare(&expected, &bytes));
        assert_eq!(matched, index == 0);
        if index == 0 {
            assert_eq!((cost.allocations, cost.reallocations), (0, 0));
        } else {
            // serde_json owns small error diagnostics; no rejected18KiB text
            // or Host catalog is copied into a decoded owner.
            assert!(cost.peak_bytes < 512, "case {index}: {}", cost.peak_bytes);
        }
    }
}
#[test]
fn unsupported_expected_contracts_refuse_preparation_explicitly() {
    let mut expected = host();
    expected.resources[0].compute = Some(ComputePoolContract {
        service_guarantee: ComputeServiceGuarantee::Shared,
        architecture_base_id: "base/compute".into(),
        architecture_base_kind: ArchitectureBaseKind::HostedOs,
        topology_groups: vec![],
        protected_preemption: None,
    });
    assert!(!bounded_host_advertisement_comparison_supported(&expected));
    assert!(HostAdvertisementEqualitySeed::new(&expected).is_none());
}
#[test]
fn duplicate_known_fields_remain_invalid() {
    let expected = host();
    let mut bytes = serde_json::to_vec(&expected).unwrap();
    bytes.pop();
    bytes.extend_from_slice(b",\"host_id\":\"host/fixture\"}");
    assert!(serde_json::from_slice::<HostAdvertisement>(&bytes).is_err());
    assert!(!compare(&expected, &bytes));
}
