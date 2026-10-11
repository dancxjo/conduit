//! Checked Source policy conformance below native USB physics.
use super::*;
use conduit_core::*;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};

fn package() -> ProtocolSourcePackage {
    let reference = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    ProtocolSourcePackage::compile(
        include_str!("../../../../plots/usb/control-reuse.conduit").into(),
        &[
            ProtocolSpecializationRequest::SeededUntil {
                value: reference("UsbControlReuseState"),
            },
            ProtocolSpecializationRequest::FeedbackZip {
                left: reference("UsbControlReuseState"),
                right: reference("UsbControlResult"),
            },
        ],
    )
    .unwrap()
}
fn program(package: &ProtocolSourcePackage, entry: &str) -> PortableExpressionProgram {
    let entry =
        PreparedProtocolEntry::prepare(&serde_json::to_vec(package).unwrap(), entry).unwrap();
    let gears = &entry.expanded().expanded.gears;
    assert_eq!(gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &gears[0].configuration[0].value else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn zero(ty: &StructuredInfoType) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Leaf(kind) => {
            let length = match kind.as_str() {
                "value/u64" => 8,
                "value/u16" => 2,
                EMPTY_INFO_ID => 0,
                _ => 1,
            };
            StructuredInfoValue::leaf(ty.clone(), vec![0; length]).unwrap()
        }
        StructuredInfoTypeShape::Sequence { .. } => {
            StructuredInfoValue::sequence(ty.clone(), vec![]).unwrap()
        }
        StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(field.name(), zero(field.value_type())).unwrap()
                })
                .collect(),
        )
        .unwrap(),
        _ => panic!("unexpected fixture shape"),
    }
}
fn state(ty: &StructuredInfoType, count: u64, terminal: bool) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                let value = match field.name() {
                    "count" => StructuredInfoValue::leaf(
                        field.value_type().clone(),
                        count.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                    "terminal" => StructuredInfoValue::leaf(
                        field.value_type().clone(),
                        vec![u8::from(terminal)],
                    )
                    .unwrap(),
                    _ => zero(field.value_type()),
                };
                StructuredFieldValue::new(field.name(), value).unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn observed(bytes: &[u8]) -> (u64, bool) {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!()
    };
    let leaf = |name: &str| {
        let value = fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value();
        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
            panic!()
        };
        bytes.to_vec()
    };
    (
        u64::from_le_bytes(leaf("count").try_into().unwrap()),
        leaf("terminal") == [1],
    )
}
fn disposition(ty: &StructuredInfoType, tag: &str) -> Vec<u8> {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!()
    };
    let payload = zero(
        cases
            .iter()
            .find(|case| case.tag() == tag)
            .unwrap()
            .payload_type(),
    );
    StructuredInfoValue::variant(ty.clone(), tag, payload)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

#[test]
fn finite_reuse_checks_one_physical_control_placement_and_exact_specializations() {
    let package = package();
    assert_eq!(package.specializations.len(), 2);
    let entry =
        PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "usb-control-reuse")
            .unwrap();
    assert_eq!(
        entry
            .expanded()
            .expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == crate::usb_base::control_contract::CONTROL_KIND)
            .count(),
        1
    );
    assert!(entry.input_schema(&port_id("seed")).is_some());
    assert!(entry.output_schema(&port_id("ended")).is_some());
}

#[test]
fn invalid_seed_counts_and_terminal_seeds_gate_before_effects() {
    let package = package();
    let initialize = program(&package, "usb-control-reuse-initialize");
    let route = program(&package, "usb-control-reuse-route");
    for (count, terminal) in [
        (0, false),
        (0, true),
        (1, false),
        (64, false),
        (65, false),
        (u64::MAX, false),
    ] {
        let initialized = initialize
            .evaluate(&state(&initialize.input_type, count, terminal))
            .unwrap();
        assert_eq!(observed(&initialized), (count, terminal || count != 0));
        let routed =
            StructuredInfoValue::from_canonical_bytes(&route.evaluate(&initialized).unwrap())
                .unwrap();
        let StructuredInfoValueShape::Variant { tag, .. } = routed.shape() else {
            panic!()
        };
        assert_eq!(
            tag,
            if count == 0 && !terminal {
                "running"
            } else {
                "stopped"
            }
        );
    }
}

#[test]
fn exact_source_updates_close_after_64_completions_and_on_typed_failures_without_growth() {
    let package = package();
    let update = program(&package, "usb-control-reuse-update");
    let ProtocolSpecialization::FeedbackZip { left, right } = &package.specializations[1] else {
        panic!()
    };
    let mut pair =
        PreparedTypedTuplePairEncoder::new(left.schema.clone(), 4096, right.schema.clone(), 4096)
            .unwrap();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&update).unwrap();
    let capacity = evaluator.output_capacity();
    let completed = disposition(&right.schema, "completed");
    // Inert canonical fixtures supply transport observations; this is no native proof.
    let inputs: Vec<_> = (0..64)
        .map(|count| {
            pair.encode(&state(&left.schema, count, false), &completed)
                .unwrap()
                .to_vec()
        })
        .collect();
    let allocations = crate::test_allocations::allocations(|| {
        for input in &inputs {
            assert!(evaluator.evaluate(input).is_ok());
        }
    });
    assert_eq!(allocations, 0);
    for (count, input) in inputs.iter().enumerate() {
        assert_eq!(
            observed(evaluator.evaluate(input).unwrap()),
            (count as u64 + 1, count == 63)
        );
    }
    assert_eq!(evaluator.output_capacity(), capacity);
    let route = program(&package, "usb-control-reuse-route");
    let final_state = evaluator.evaluate(inputs.last().unwrap()).unwrap();
    let closed =
        StructuredInfoValue::from_canonical_bytes(&route.evaluate(final_state).unwrap()).unwrap();
    assert!(matches!(
        closed.shape(),
        StructuredInfoValueShape::Variant { tag: "stopped", .. }
    ));
    for tag in ["stalled", "provider-lost", "unsupported"] {
        let failure = disposition(&right.schema, tag);
        let input = pair
            .encode(&state(&left.schema, 7, false), &failure)
            .unwrap();
        assert_eq!(observed(evaluator.evaluate(input).unwrap()), (7, true));
    }
}

#[test]
fn finite_reuse_reaches_ordinary_native_source_plan_admission() {
    use crate::protocol_source::NativeProtocolPreparationReason as Reason;
    use crate::usb_base::{
        control_contract::*, control_factory::*, installation::UsbNativeIdentity,
    };
    let entry = PreparedProtocolEntry::prepare(
        &serde_json::to_vec(&package()).unwrap(),
        "usb-control-reuse",
    )
    .unwrap();
    // Even without a redundant Fore envelope annotation, the structured
    // schema and native Root ceiling reject oversized seed frames.
    let seed_schema = entry.input_schema(&port_id("seed")).unwrap();
    let validator = PreparedStructuredValueValidator::new(
        &seed_schema,
        crate::protocol_source::MAXIMUM_PROTOCOL_FORE_BYTES as usize,
    )
    .unwrap();
    assert!(validator.validate(&vec![0; 4097]).is_err());
    let identity = UsbNativeIdentity {
        host_id: "reuse/host".into(),
        boot_id: "reuse/boot".into(),
        base_id: "reuse/base".into(),
        provider_instance_id: "reuse/provider".into(),
        provider_generation: 7,
        resource_pool_id: "reuse/dma".into(),
        resource_generation_id: ResourceGenerationId("reuse/dma/7".into()),
        envelope_id: "reuse/envelope".into(),
        artifact_id: entry.artifact_id().clone(),
    };
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    identity
        .append_to_advertisement(&mut host, &ControlContract::prepare().unwrap())
        .map_err(Reason::Usb)
        .unwrap();
    entry
        .publish_pure_backs(&mut host)
        .map_err(Reason::Source)
        .unwrap();
    let grant = AuthorityGrant {
        grant_id: "reuse/grant".into(),
        contract_id: CONTROL_AUTHORITY.into(),
        host_call_contract_id: CONTROL_CALL.into(),
        subject_kind: CONTROL_KIND.into(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: CONTROL_IMPLEMENTATION.into(),
    };
    let hosts = [host];
    let placements = entry.placements(&hosts).map_err(Reason::Source).unwrap();
    let empty = alloc::collections::BTreeMap::new();
    let prepared = entry
        .plan(
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &empty,
                line_candidates: &alloc::collections::BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 512,
                authority_grants: &[grant],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(Reason::Source)
        .unwrap();
    let plan = &prepared.artifact().definition().internal_plan;
    let seed_boundary = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.fore_ports)
        .find(|boundary| {
            boundary.front_port_id == port_id("seed") && boundary.track == ConnectionTrack::Payload
        })
        .unwrap();
    assert!(seed_boundary.byte_capacity > 0);
    assert!(seed_boundary.byte_capacity <= crate::protocol_source::MAXIMUM_PROTOCOL_FORE_BYTES);
    assert_eq!(
        prepared
            .artifact()
            .definition()
            .internal_plan
            .fragments
            .len(),
        1
    );
}
