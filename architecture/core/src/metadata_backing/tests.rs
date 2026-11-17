//! Payload descriptors preserve identities and distinguish unsupported ownership.
use super::*;
use alloc::{string::String, vec};

fn kind() -> Kind {
    Kind {
        startup_parameters: vec![FrontStartupParameter {
            name: "label".into(),
            value_type: kind_id(TEXT_INFO_ID),
            has_default: true,
        }],
        shorthand: Some((port_id("in"), port_id("out"))),
        kind_id: kind_id("test/metadata-kind"),
        kind_contract_revision: "test/metadata-kind@1".into(),
        inputs: vec![port("in", PortDirection::Input)],
        outputs: vec![port("out", PortDirection::Output)],
        configuration: vec![KindConfigurationField {
            key: "label".into(),
            default_value: ConfigurationValue::Text("Label".into()),
            rule: KindConfigurationRule::DataLabel { maximum: 256 },
        }],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 256,
        },
    }
}
fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(TEXT_INFO_ID),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: Some(kind_id(TEXT_INFO_ID)),
    }
}
#[test]
fn kind_metadata_counts_empty_vector_spare_and_preserves_fore_identity() {
    let mut value = kind();
    let before = value.retained_backing_bytes().unwrap();
    let identity = compute_checked_front_fingerprint(&value.checked_front());
    let capacity = value.semantic_laws.capacity();
    value.semantic_laws.reserve(9);
    let additional = (value.semantic_laws.capacity() - capacity) * size_of::<KindSemanticLaw>();
    assert_eq!(value.retained_backing_bytes(), Some(before + additional));
    assert_eq!(
        compute_checked_front_fingerprint(&value.checked_front()),
        identity
    );
    value
        .semantic_laws
        .push(KindSemanticLaw::FlowEach(FlowEachSemanticLaw {
            input_port_id: port_id("in"),
            output_port_id: port_id("out"),
            maximum_items: 1,
        }));
    assert_eq!(value.retained_backing_bytes(), None);
}
#[test]
fn checked_front_counts_private_capacities_without_serializing() {
    let front = |capacity: usize| {
        let mut startup = Vec::with_capacity(capacity);
        startup.push(FrontStartupParameter {
            name: "label".into(),
            value_type: kind_id(TEXT_INFO_ID),
            has_default: true,
        });
        let mut inputs = Vec::with_capacity(capacity);
        inputs.push(port("in", PortDirection::Input));
        CheckedFront::new(
            startup,
            inputs,
            vec![port("out", PortDirection::Output)],
            Some((port_id("in"), port_id("out"))),
        )
    };
    let lean = front(1);
    let padded = front(7);
    assert_eq!(lean, padded);
    assert_eq!(
        compute_checked_front_fingerprint(&lean),
        compute_checked_front_fingerprint(&padded)
    );
    assert_eq!(
        padded.retained_backing_bytes().unwrap() - lean.retained_backing_bytes().unwrap(),
        6 * (size_of::<FrontStartupParameter>() + size_of::<PortDescriptor>())
    );
    let before = padded.retained_backing_bytes().unwrap();
    let empty = Vec::<ResourcePortContract>::with_capacity(5);
    let bytes = empty.capacity() * size_of::<ResourcePortContract>();
    let padded = padded.with_resource_ports(empty);
    assert_eq!(padded.retained_backing_bytes(), Some(before + bytes));
}
#[test]
fn structured_field_uses_owned_cow_capacity_and_exact_profile_rule() {
    let ty = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap();
    let profile = ty.profile().unwrap().value_kind().clone();
    let encoded = StructuredInfoValue::leaf(ty, b"hello".to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let mut bytes = Vec::with_capacity(encoded.len() + 79);
    bytes.extend_from_slice(&encoded);
    let structured = StructuredConfigurationValue::new(profile.clone(), bytes).unwrap();
    let mut key = String::with_capacity(57);
    key.push_str("recipe");
    let expected =
        key.capacity() + structured.owned_backing_bytes().unwrap() + profile.0.capacity();
    let mut field = KindConfigurationField {
        key,
        default_value: ConfigurationValue::Structured(structured),
        rule: KindConfigurationRule::Structured { profile },
    };
    assert_eq!(field.retained_backing_bytes(), Some(expected));
    field.default_value = ConfigurationValue::Unit(UnitConfigurationValue::parse("m").unwrap());
    assert_eq!(field.retained_backing_bytes(), None);
}
#[test]
fn resource_and_value_contract_laws_retain_nested_payloads() {
    let mut value = kind();
    let contract = CheckedValueContract::new(
        kind_id(TEXT_INFO_ID),
        128,
        vec![ValueConstraint::CanonicalMembership {
            members: vec![Vec::with_capacity(41)],
            negated: true,
        }],
    )
    .unwrap();
    value.semantic_laws = vec![
        KindSemanticLaw::ResourcePorts(vec![ResourcePortContract {
            port_id: port_id("surface"),
            class_id: ResourceClassId::from("face/surface"),
            ownership: ResourcePortOwnership::Move,
            lifecycle: ResourcePortLifecycle::Play,
            mobility: ResourcePortMobility::HostLocal,
        }]),
        KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::Input(port_id("in")),
            contract,
        }]),
    ];
    assert!(
        value.retained_backing_bytes().unwrap()
            > value.checked_front().retained_backing_bytes().unwrap()
    );
}
#[test]
fn checked_add_refuses_overflow_without_relabeling_it_as_zero() {
    let mut bytes = Bytes(usize::MAX);
    assert_eq!(bytes.add(1), None);
    assert_eq!(bytes.0, usize::MAX);
}
