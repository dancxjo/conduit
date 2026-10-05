//! Checked class composition over an inert endpoint contract; no device claim.
use conduit_core::*;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
use conduitos::protocol_source::{PreparedProtocolSource, ProtocolSourcePackage};
use conduitos::usb_base::{
    endpoint_read_contract::{ENDPOINT_READ_KIND, EndpointReadContract},
    endpoint_read_result::{EndpointReadDisposition, PreparedEndpointReadResultEncoder},
};

fn package() -> ProtocolSourcePackage {
    let mut package = conduitos::protocol_source::usb_hid_endpoint_package().unwrap();
    package
        .source
        .push_str("\ntype ImportedHidEndpointResult = UsbEndpointReadResult\n");
    package
}
fn program(entry: &str) -> PortableExpressionProgram {
    let source = PreparedProtocolSource::prepare(package()).unwrap();
    let expanded = source.expand(entry).unwrap().expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("pure program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn forged_frame(contract: &EndpointReadContract, wire: &[u8], actual: u64, short: bool) -> Vec<u8> {
    let StructuredInfoTypeShape::Variant { cases, .. } = contract.result_type().shape() else {
        panic!("result")
    };
    let ty = cases
        .iter()
        .find(|case| case.tag() == "completed")
        .unwrap()
        .payload_type();
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("completed frame")
    };
    let field_type = |name: &str| {
        fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    let value = |name: &str, encoded: Vec<u8>| {
        StructuredFieldValue::new(
            name,
            StructuredInfoValue::leaf(field_type(name), encoded).unwrap(),
        )
        .unwrap()
    };
    let frame = StructuredInfoValue::record(
        ty.clone(),
        vec![
            value("actual", actual.to_le_bytes().to_vec()),
            value("short", vec![u8::from(short)]),
            value("wire", wire.to_vec()),
        ],
    )
    .unwrap();
    StructuredInfoValue::variant(contract.result_type().clone(), "completed", frame)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

#[test]
fn endpoint_class_graphs_preserve_byte_refinements_without_advertising_physics() {
    let source = PreparedProtocolSource::prepare(package()).unwrap();
    let imported = source
        .checked
        .native_types
        .iter()
        .find(|ty| ty.name == "ImportedHidEndpointResult")
        .unwrap();
    let wire = imported
        .value_contracts
        .iter()
        .find(|contract| contract.representation_path == "|completed.wire")
        .unwrap();
    assert_eq!(wire.contract.maximum_bytes, 2048);
    assert_eq!(wire.contract.value_kind.as_str(), "value/bytes");
    for entry in ["usb-hid-keyboard-endpoint", "usb-hid-mouse-endpoint"] {
        let expanded = source.expand(entry).unwrap();
        assert_eq!(
            expanded
                .expanded
                .gears
                .iter()
                .filter(|gear| gear.kind_id.as_str() == ENDPOINT_READ_KIND)
                .count(),
            1
        );
        let mut host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: "fixture/hid".into(),
            boot_id: "fixture/boot".into(),
            offer_generation: OfferGeneration(1),
            profile: "fixture/hid-source".into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        };
        source.publish_pure_backs(&expanded, &mut host).unwrap();
        assert!(
            host.capabilities
                .iter()
                .all(|offer| offer.kind_id.as_str() != ENDPOINT_READ_KIND)
        );
        assert!(conduit_planner::default_expanded_placements(&expanded.expanded, &[host]).is_err());
    }
}

#[test]
fn boot_report_request_is_eight_bytes_without_endpoint_selection_facts() {
    let program = program("usb-hid-endpoint-request");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let request = validate_canonical_structured_value(prepared.evaluate(&[]).unwrap()).unwrap();
    assert_eq!(
        request
            .record_field("length")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        8_u64.to_le_bytes()
    );
}

#[test]
fn endpoint_framing_keeps_actual_extent_and_distinct_transport_outcomes_without_growth() {
    let program = program("usb-hid-endpoint-frame");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let contract = EndpointReadContract::prepare().unwrap();
    let mut encoder = PreparedEndpointReadResultEncoder::new(&contract).unwrap();
    let mut cases = Vec::new();
    for actual in 0..=8_u16 {
        let wire: Vec<_> = (0..actual).map(|index| index as u8).collect();
        cases.push((
            encoder.completed(8, actual, &wire).unwrap().to_vec(),
            "frame",
            Some(wire),
        ));
    }
    for (disposition, tag) in [
        (EndpointReadDisposition::Stalled, "stalled"),
        (EndpointReadDisposition::ProviderLost, "provider-lost"),
        (EndpointReadDisposition::Unsupported, "unsupported"),
        (EndpointReadDisposition::Timeout, "timeout"),
    ] {
        cases.push((
            encoder.disposition(disposition).unwrap().to_vec(),
            tag,
            None,
        ));
    }
    for (wire, actual, short) in [
        (vec![0; 8], 8, true),
        (vec![0; 7], 7, false),
        (vec![0; 8], 7, true),
        (vec![], u64::MAX, false),
        (vec![0; 9], 9, false),
        (vec![0; 2048], 2048, false),
    ] {
        let input = forged_frame(&contract, &wire, actual, short);
        assert!(input.len() <= 4096);
        cases.push((input, "malformed", None));
    }
    for (input, tag, wire) in &cases {
        let ordinary = program.evaluate(input).unwrap();
        let output = prepared.evaluate(input).unwrap();
        assert_eq!(output, ordinary);
        super::common::tag(output, tag);
        if let Some(wire) = wire {
            let frame = validate_canonical_structured_value(output)
                .unwrap()
                .variant_payload("frame")
                .unwrap()
                .unwrap();
            assert_eq!(
                frame
                    .record_field("wire")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bytes")
                    .unwrap(),
                wire
            );
            assert_eq!(
                frame
                    .record_field("actual")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                (wire.len() as u64).to_le_bytes()
            );
        }
    }
    let capacity = prepared.output_capacity();
    let allocations = crate::allocation::allocations(|| {
        for _ in 0..1024 {
            for (input, tag, _) in &cases {
                super::common::tag(prepared.evaluate(input).unwrap(), tag);
            }
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(prepared.output_capacity(), capacity);
}

#[test]
fn class_proof_plan_retains_exact_endpoint_selection_and_source_identity() {
    use conduitos::usb_base::{
        endpoint_read_proof_plan::EndpointReadProofSubject, hid_endpoint_proof_plan,
    };
    let subject = EndpointReadProofSubject {
        host_id: "proof/host",
        boot_id: "proof/boot",
        controller_base_id: "proof/controller",
        device_instance_id: "proof/device",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
        endpoint_dci: 3,
        endpoint_epoch: 1,
    };
    for entry in ["usb-hid-keyboard-endpoint", "usb-hid-mouse-endpoint"] {
        let retained = hid_endpoint_proof_plan::plan(&subject, entry).unwrap();
        let artifact = retained.artifact();
        let plan = &artifact.definition().internal_plan;
        assert!(verify_plan(plan));
        assert_eq!(plan.source_document_id, artifact.identity().source);
        let fragment = &plan.fragments[0];
        assert!(
            fragment
                .fore_ports
                .iter()
                .all(|port| port.item_capacity == 1 && port.byte_capacity <= 4096)
        );
        assert_eq!(fragment.host_id.as_str(), subject.host_id);
        assert_eq!(fragment.boot_id.as_str(), subject.boot_id);
        let endpoint = fragment
            .placements
            .iter()
            .find(|gear| gear.kind_id.as_str() == ENDPOINT_READ_KIND)
            .unwrap();
        assert_eq!(
            endpoint.base.as_ref().unwrap().base_id.as_str(),
            subject.controller_base_id
        );
        assert_eq!(endpoint.resources.len(), 1);
        assert_eq!(endpoint.authority.len(), 1);
        assert!(
            retained
                .prepare_pure(conduit_composite::KernelCompositeSignStorage {
                    additional_local_items: 0,
                    additional_remote_items: 0
                })
                .is_err()
        );
    }
    assert!(hid_endpoint_proof_plan::plan(&subject, "usb-hid-endpoint-request").is_err());
    let invalid = EndpointReadProofSubject {
        endpoint_dci: 2,
        ..subject
    };
    assert!(hid_endpoint_proof_plan::plan(&invalid, "usb-hid-keyboard-endpoint").is_err());
}

#[test]
fn sole_kernel_constructs_and_retains_the_endpoint_call_for_native_admission() {
    use conduitos::usb_base::{
        endpoint_read_proof_plan::EndpointReadProofSubject,
        hid_endpoint_proof_kernel::PreparedHidEndpointProofKernel, hid_endpoint_proof_plan,
    };
    let subject = EndpointReadProofSubject {
        host_id: "proof/host",
        boot_id: "proof/boot",
        controller_base_id: "proof/controller",
        device_instance_id: "proof/device",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
        endpoint_dci: 3,
        endpoint_epoch: 1,
    };
    for entry in ["usb-hid-keyboard-endpoint", "usb-hid-mouse-endpoint"] {
        let artifact = hid_endpoint_proof_plan::plan(&subject, entry).unwrap();
        let read = artifact
            .artifact()
            .definition()
            .external_capability
            .inputs
            .iter()
            .find(|port| port.port_id.as_str() == "read")
            .unwrap()
            .clone();
        let mut play = PreparedHidEndpointProofKernel::prepare(
            artifact,
            conduit_composite::KernelCompositeSignStorage {
                additional_local_items: 4096,
                additional_remote_items: 4096,
            },
        )
        .unwrap();
        play.kernel_mut().start().unwrap();
        let unit = ValuePayload {
            value_kind: read.value_kind,
            encoded: Vec::new(),
        };
        play.kernel_mut()
            .admit_input(&read.port_id, 0, &unit)
            .unwrap();
        let mut endpoint_request = None;
        let allocations = crate::allocation::allocations(|| {
            for _ in 0..256 {
                play.kernel_mut().step().unwrap();
                if let Some(request) = play.kernel_mut().next_host_request() {
                    if !play.service_pure_call(&request).unwrap() {
                        assert!(!play.service_pure_call(&request).unwrap());
                        endpoint_request = Some(request);
                        break;
                    }
                }
            }
        });
        assert_eq!(allocations, 0);
        let request = endpoint_request.expect("exact physical handoff");
        let obligation = play.kernel_mut().host_request_obligation(&request).unwrap();
        assert_eq!(
            obligation.requirement.contract_id.as_str(),
            conduitos::usb_base::endpoint_read_contract::ENDPOINT_READ_CALL
        );
        let host = obligation.host.clone();
        let resources = obligation.resources.clone();
        let authorities = obligation.authorities.clone();
        // Kernel admission is tested without invoking an actual device owner.
        let admitted = play
            .kernel_mut()
            .admit_host_request(&request, &host, &resources, &authorities)
            .unwrap();
        let input = play.kernel_mut().host_request_input(&admitted).unwrap();
        let frame = validate_canonical_structured_value(input).unwrap();
        assert_eq!(
            frame
                .record_field("length")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            8_u64.to_le_bytes()
        );
        play.kernel_mut().cancel().unwrap();
    }
}
