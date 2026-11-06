//! Existing `when` meaning uses the ordinary native computation path.
use conduit_core::*;
use conduitos::protocol_source::*;
use std::collections::BTreeMap;
#[test]
fn native_when_forwards_exact_input_or_no_value_without_play_allocations() {
    let source = "type Item = {\n value: U64\n}\nplot filtered (\n input: Item...| <= 4096B >> output: Item...| <= 4096B\n) {\n input >> when(.value > 1) >> output\n}\n";
    let bytes =
        serde_json::to_vec(&ProtocolSourcePackage::compile(source.into(), &[]).unwrap()).unwrap();
    let entry = PreparedProtocolEntry::prepare(&bytes, "filtered").unwrap();
    let input_type = entry.input_schema(&port_id("input")).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = input_type.shape() else {
        panic!("record")
    };
    let field_type = fields[0].value_type().clone();
    let frames: Vec<_> = (0_u64..4)
        .map(|n| {
            StructuredInfoValue::record(
                input_type.clone(),
                vec![
                    StructuredFieldValue::new(
                        "value",
                        StructuredInfoValue::leaf(field_type.clone(), n.to_le_bytes().to_vec())
                            .unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
            .canonical_bytes()
            .unwrap()
        })
        .collect();
    let mut host = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/filter-host".into(),
        boot_id: "fixture/filter-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/native-filter".into(),
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
    let mut run = conduitos::usb_base::hid_source_kernel::PreparedUsbSourceKernel::prepare_pure(
        artifact,
        conduit_composite::KernelCompositeSignStorage::default(),
    )
    .unwrap();
    let front = run.kernel_mut().definition().boundary.clone();
    let input = front.input_fronts[0].external_port.clone();
    let output = front.output_fronts[0].external_port.clone();
    let incoming: Vec<_> = frames
        .iter()
        .map(|bytes| ValuePayload {
            value_kind: input.value_kind.clone(),
            encoded: bytes.clone(),
        })
        .collect();
    let mut received = ValuePayload {
        value_kind: output.value_kind,
        encoded: Vec::with_capacity(4096),
    };
    let mut seen = 0;
    let mut complete = false;
    let allocations = super::allocation::allocations(|| {
        run.kernel_mut().start().unwrap();
        for (index, frame) in incoming.iter().enumerate() {
            run.kernel_mut()
                .admit_input(&input.port_id, index as u64, frame)
                .unwrap();
            if index == 3 {
                run.kernel_mut().close_input(&input.port_id).unwrap();
            }
            for _ in 0..64 {
                let status = run.kernel_mut().step().unwrap();
                if let Some(request) = run.kernel_mut().next_host_request() {
                    assert!(run.service_pure_call(&request).unwrap());
                }
                if let Some(sequence) = run
                    .kernel_mut()
                    .output_into(&output.port_id, &mut received)
                    .unwrap()
                {
                    assert_eq!(received.encoded, frames[seen + 2]);
                    seen += 1;
                    run.kernel_mut()
                        .complete_output(&output.port_id, sequence)
                        .unwrap();
                }
                if status == conduit_composite::KernelCompositeStatus::Complete {
                    complete = true;
                    break;
                }
            }
        }
    });
    assert_eq!(seen, 2);
    assert!(complete);
    assert_eq!(allocations, 0);
}
