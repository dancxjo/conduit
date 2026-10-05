use conduit_composite::{
    KernelCompositeSignStorage, KernelCompositeStatus, KernelCompositeTerminal,
};
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduitos::protocol_source::PreparedProtocolEntry;
use std::collections::BTreeMap;

#[test]
fn source_keyboard_state_drains_64_reports_in_order_under_pressure_without_allocations() {
    let entry = PreparedProtocolEntry::prepare(
        &serde_json::to_vec(&super::lifecycle::package()).unwrap(),
        "usb-hid-keyboard-lifecycle",
    )
    .unwrap();
    let schema = entry.input_schema(&PortId::from("command")).unwrap();
    let StructuredInfoTypeShape::Variant { cases, .. } = schema.shape() else {
        panic!("command");
    };
    let report_type = cases
        .iter()
        .find(|c| c.tag() == "report")
        .unwrap()
        .payload_type()
        .clone();
    let unit_type = cases
        .iter()
        .find(|c| c.tag() == "finish")
        .unwrap()
        .payload_type()
        .clone();
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/keyboard-source-host".into(),
        boot_id: "fixture/keyboard-source-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/keyboard-source".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    entry.publish_pure_backs(&mut host).unwrap();
    let hosts = [host];
    let placements = entry.placements(&hosts).unwrap();
    let artifact = entry
        .plan(
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap();
    let run = artifact
        .prepare_pure(KernelCompositeSignStorage {
            additional_local_items: 60000,
            additional_remote_items: 60000,
        })
        .unwrap();
    let command = run
        .kernel()
        .definition()
        .boundary
        .input_fronts
        .iter()
        .find(|p| p.external_port.port_id.as_str() == "command")
        .unwrap()
        .external_port
        .clone();
    let begin = run
        .kernel()
        .definition()
        .boundary
        .input_fronts
        .iter()
        .find(|p| p.external_port.port_id.as_str() == "begin")
        .unwrap()
        .external_port
        .clone();
    let output = run.kernel().definition().boundary.output_fronts[0]
        .external_port
        .clone();
    let mut inputs = Vec::new();
    let mut expected = Vec::new();
    let mut previous = (0, [0; 6]);
    for index in 0..64 {
        let current = if index % 2 == 0 {
            (255, [4, 5, 6, 7, 8, 9])
        } else {
            (0, [10, 11, 12, 13, 14, 15])
        };
        expected.extend(super::transitions::expected(previous, current));
        let payload = super::transitions::report(report_type.clone(), current.0, current.1);
        inputs.push(ValuePayload {
            value_kind: command.value_kind.clone(),
            encoded: StructuredInfoValue::variant(schema.clone(), "report", payload)
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        });
        previous = current;
    }
    inputs.push(ValuePayload {
        value_kind: command.value_kind.clone(),
        encoded: StructuredInfoValue::variant(
            schema,
            "finish",
            StructuredInfoValue::leaf(unit_type, vec![]).unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap(),
    });
    let begin_value = ValuePayload {
        value_kind: begin.value_kind.clone(),
        encoded: vec![],
    };
    let mut value = ValuePayload {
        value_kind: output.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    assert_eq!(expected.len(), 1274);
    let mut held = Vec::with_capacity(4096);
    let mut run = run;
    let mut next_input = 0;
    let mut next_output = 0;
    let mut pressure = 0;
    let allocations = crate::allocation::allocations(|| {
        run.start().unwrap();
        assert_eq!(
            run.admit_input(&begin.port_id, 0, &begin_value).unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 0 }
        );
        run.close_input(&begin.port_id).unwrap();
        let mut complete = false;
        for _ in 0..131072 {
            if next_input < inputs.len() {
                match run
                    .admit_input(&command.port_id, next_input as u64, &inputs[next_input])
                    .unwrap()
                {
                    RemoteIngressOutcome::Accepted { .. } => {
                        next_input += 1;
                        if next_input == inputs.len() {
                            run.close_input(&command.port_id).unwrap();
                        }
                    }
                    RemoteIngressOutcome::Full { .. } => pressure += 1,
                }
            }
            let status = run.step().unwrap();
            if let Some(sequence) = run.output_into(&output.port_id, &mut value).unwrap() {
                assert_eq!(sequence, next_output as u64);
                if next_output == 0 {
                    held.extend_from_slice(&value.encoded);
                    for _ in 0..128 {
                        run.step().unwrap();
                        assert_eq!(
                            run.output_into(&output.port_id, &mut value).unwrap(),
                            Some(0)
                        );
                        assert_eq!(value.encoded, held);
                    }
                }
                let event = validate_canonical_structured_value(&value.encoded).unwrap();
                let octet = |name| {
                    event
                        .record_field(name)
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u8")
                        .unwrap()[0]
                };
                let pressed = event
                    .record_field("pressed")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/bool")
                    .unwrap()[0]
                    != 0;
                assert_eq!(
                    (octet("usage"), pressed, octet("modifiers")),
                    expected[next_output]
                );
                next_output += 1;
                run.complete_output(&output.port_id, sequence).unwrap();
            }
            if status == KernelCompositeStatus::Complete {
                complete = true;
                break;
            }
        }
        assert!(complete);
        assert_eq!(next_input, inputs.len());
        assert_eq!(next_output, expected.len());
        assert!(pressure > 0);
        assert_eq!(
            run.output_terminal_into(&output.port_id, &mut value)
                .unwrap(),
            Some(KernelCompositeTerminal::Normal)
        );
    });
    assert_eq!(allocations, 0);
}
