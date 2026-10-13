//! Generic typed merge instances preserve the reviewed finite Back and bounds.
use super::*;
use crate::protocol_source::{
    PreparedProtocolEntry, ProtocolSourcePackage, ProtocolSpecializationRequest,
    ProtocolValueReference,
};
use conduit_core::*;

fn reference(name: &str, maximum_bytes: u32) -> ProtocolValueReference {
    ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes,
    }
}
fn source() -> String {
    "type First = {\n value: U64\n}\ntype Second = {\n flag: Boolean\n}\nplot first (\n >> left: First...|\n >> right: First...|\n result: First...| >>\n) {\n merge: FirstMerge\n left >> merge.left\n right >> merge.right\n merge.merged >> result\n}\nplot second (\n >> left: Second...|\n >> right: Second...|\n result: Second...| >>\n) {\n merge: SecondMerge\n left >> merge.left\n right >> merge.right\n merge.merged >> result\n}".into()
}
#[test]
fn two_merge_instance_schemas_round_trip_without_overwriting_family_catalog() {
    let requests = [
        ProtocolSpecializationRequest::MergeInstance {
            alias: "FirstMerge".into(),
            value: reference("First", 4096),
        },
        ProtocolSpecializationRequest::MergeInstance {
            alias: "SecondMerge".into(),
            value: reference("Second", 4096),
        },
    ];
    let package = ProtocolSourcePackage::compile(source(), &requests).unwrap();
    let bytes = serde_json::to_vec(&package).unwrap();
    let mut identities = alloc::vec![];
    for name in ["first", "second"] {
        let entry = PreparedProtocolEntry::prepare(&bytes, name).unwrap();
        let gears = &entry.expanded().expanded.gears;
        assert_eq!(gears.len(), 1);
        let gear = &gears[0];
        assert!(
            gear.kind_id
                .as_str()
                .starts_with("flow/merge/finite/typed-")
        );
        assert_eq!(
            gear.kind_contract_revision.as_str(),
            "conduit.flow/merge-finite@2"
        );
        assert_eq!(gear.inputs.len(), 2);
        assert_eq!(gear.outputs.len(), 1);
        identities.push(gear.kind_id.clone());
    }
    assert_ne!(identities[0], identities[1]);
}
#[test]
fn merge_instance_alias_and_bounds_refuse_before_catalog_admission() {
    for alias in ["", "1Invalid", "First\nwith other as Injected", "x/y"] {
        assert!(
            ProtocolSourcePackage::compile(
                source(),
                &[ProtocolSpecializationRequest::MergeInstance {
                    alias: alias.into(),
                    value: reference("First", 4096)
                }]
            )
            .is_err()
        );
    }
    for maximum_bytes in [0, 32769] {
        assert!(
            ProtocolSourcePackage::compile(
                source(),
                &[ProtocolSpecializationRequest::MergeInstance {
                    alias: "FirstMerge".into(),
                    value: reference("First", maximum_bytes)
                }]
            )
            .is_err()
        );
    }
    let package = ProtocolSourcePackage::compile(
        source(),
        &[ProtocolSpecializationRequest::MergeInstance {
            alias: "FirstMerge".into(),
            value: reference("First", 4097),
        }],
    )
    .unwrap();
    assert!(crate::protocol_source::PreparedProtocolSource::prepare(package).is_err());
}
#[test]
fn typed_merge_and_legacy_coexist_with_exact_bounded_offer_identities() {
    let schema = StructuredInfoType::record(
        kind_id("test/merge-value@1"),
        alloc::vec![
            StructuredFieldType::new(
                "value",
                StructuredInfoType::leaf(kind_id("value/u64")).unwrap()
            )
            .unwrap()
        ],
    )
    .unwrap();
    let value = CheckedValueContract::new(
        schema.profile().unwrap().value_kind().clone(),
        4096,
        alloc::vec![],
    )
    .unwrap();
    let mut factory = crate::flow_merge_finite::FlowMergeFiniteOperationFactory::default();
    let legacy = factory.install(&value, &schema).unwrap();
    let typed = factory.install_instance(&value, &schema).unwrap();
    assert_ne!(legacy.kind_id, typed.kind_id);
    assert_ne!(legacy.capability_id, typed.capability_id);
    assert_eq!(legacy.inputs, typed.inputs);
    assert_eq!(legacy.outputs, typed.outputs);
    assert_eq!(legacy.semantic_contract, typed.semantic_contract);
    assert_eq!(legacy.limits, typed.limits);
    assert!(
        typed.host_calls.is_empty()
            && typed.resource_requirements.is_empty()
            && typed.authority_requirements.is_empty()
    );
    assert!(factory.install_instance(&value, &schema).is_err());
    let mut wider = value.clone();
    wider.maximum_bytes = 4097;
    assert!(factory.install_instance(&wider, &schema).is_err());
    let mut unsupported = value.clone();
    unsupported.constraints.push(ValueConstraint::ByteLength {
        minimum: 0,
        maximum: 4096,
    });
    assert!(factory.install_instance(&unsupported, &schema).is_err());
    let mut narrow = value.clone();
    narrow.maximum_bytes = 2048;
    let second = factory.install_instance(&narrow, &schema).unwrap();
    assert_ne!(typed.kind_id, second.kind_id);
    assert_ne!(typed.capability_id, second.capability_id);
    let wrong = StructuredInfoType::leaf(kind_id("value/bool")).unwrap();
    assert!(factory.install_instance(&value, &wrong).is_err());
}

#[test]
fn merge_alias_cannot_rebind_a_different_instance_or_generic_family() {
    let first = ProtocolSpecializationRequest::MergeInstance {
        alias: "FirstMerge".into(),
        value: reference("First", 4096),
    };
    for other in [
        ProtocolSpecializationRequest::MergeInstance {
            alias: "FirstMerge".into(),
            value: reference("Second", 4096),
        },
        ProtocolSpecializationRequest::SeededUntilInstance {
            alias: "FirstMerge".into(),
            value: reference("Second", 4096),
        },
    ] {
        assert!(ProtocolSourcePackage::compile(source(), &[first.clone(), other]).is_err());
    }
}
#[test]
fn same_implementation_resolves_exact_retained_merge_capability_and_refuses_effects() {
    use conduit_composite::KernelOperationFactory;
    let schema = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let mut factory = crate::flow_merge_finite::FlowMergeFiniteOperationFactory::default();
    let make =
        |maximum| CheckedValueContract::new(kind_id("value/u64"), maximum, alloc::vec![]).unwrap();
    let first = factory.install_instance(&make(8), &schema).unwrap();
    let second = factory.install_instance(&make(16), &schema).unwrap();
    for offer in [&first, &second] {
        let (mut fragment, _, _, _) =
            crate::usb_base::endpoint_read_owner::tests::fixture::selected();
        let gear = &mut fragment.placements[0];
        gear.kind_id = offer.kind_id.clone();
        gear.kind_contract_revision = offer.kind_contract_revision.clone();
        gear.capability_id = offer.capability_id.clone();
        gear.implementation_id = offer.implementation.implementation_id.clone();
        gear.execution_profile_id = offer.implementation.execution_profile_id.clone();
        gear.artifact_id = offer.implementation.artifact_id.clone();
        gear.inputs = offer.inputs.clone();
        gear.outputs = offer.outputs.clone();
        gear.semantic_contract = offer.semantic_contract.clone();
        gear.limits = offer.limits.clone();
        gear.base = None;
        gear.resources.clear();
        gear.authority.clear();
        gear.host_calls.clear();
        gear.configuration.clear();
        gear.pool_references.clear();
        let budget = factory.budget(gear).unwrap();
        assert_eq!(budget.host_requests, 0);
        assert_eq!(
            budget.maximum_value_bytes,
            if offer.capability_id == first.capability_id {
                8
            } else {
                16
            }
        );
        let mut wrong = gear.clone();
        wrong.kind_id = if offer.kind_id == first.kind_id {
            second.kind_id.clone()
        } else {
            first.kind_id.clone()
        };
        assert!(factory.budget(&wrong).is_err());
        wrong = gear.clone();
        wrong.host_calls.push(HostCallRequirement {
            contract_id: "forged/effect".into(),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: 1,
        });
        assert!(factory.budget(&wrong).is_err());
    }
}
