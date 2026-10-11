//! Selected storage Back/Source evidence; no storage authority or USB fixture.
use conduit_core::*;
use conduit_plot::PortableExpressionProgram;
use conduitos::protocol_source::*;
fn package() -> ProtocolSourcePackage {
    let (header, body) = include_str!("../plots/usb/bot-storage-back.conduit")
        .split_once("\n\n")
        .unwrap();
    let reference = |name: &str, maximum_bytes| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes,
    };
    let source = format!(
        "{header}\n{}\n{}\n{}\n{}\n{}\n{body}\nplot block-storage-entrance (\n    request: BlockReadRequest...| <= 4096B >> result: BlockReadResult...| <= 4096B\n) {{\n    provider: storage/block-read\n    request >> provider.request\n    provider.result >> result\n}}\n",
        include_str!("../../../semantics/storage/block-read-types.conduit"),
        include_str!("../plots/usb/bot-types.conduit"),
        include_str!("../plots/usb/bot-profile.conduit"),
        include_str!("../plots/usb/bot-storage-types.conduit"),
        include_str!("../plots/usb/bot-storage-observations.conduit")
    );
    let mut package = ProtocolSourcePackage::compile(
        source,
        &[
            ProtocolSpecializationRequest::Singleton {
                value: reference("BotStorageEvent", 4096),
            },
            ProtocolSpecializationRequest::Concat {
                value: reference("BotStorageEvent", 4096),
            },
            ProtocolSpecializationRequest::SeededUntil {
                value: reference("BotStorageState", 4096),
            },
            ProtocolSpecializationRequest::FeedbackZip {
                left: reference("BotStorageState", 4096),
                right: reference("BotStorageEvent", 4096),
            },
            ProtocolSpecializationRequest::Zip {
                left: reference("UsbControlResult", 4096),
                right: reference("BotEvent", 4096),
            },
            ProtocolSpecializationRequest::Merge {
                value: reference("BotStorageEvent", 4096),
            },
        ],
    )
    .unwrap();
    let prepared = PreparedProtocolSource::prepare(package.clone()).unwrap();
    let selected = prepared
        .checked
        .plots
        .iter()
        .find(|plot| plot.name == "bot-storage-provider")
        .unwrap();
    package.backs.push(ProtocolBackSelection {
        kind_id: kind_id("storage/block-read"),
        plot_name: selected.name.clone(),
        source_document_id: prepared.checked.source_document_id.clone(),
        checked_plot_id: selected.checked_plot_id.clone(),
    });
    package
}
#[test]
fn selected_storage_back_preserves_exact_fore_and_owns_one_usb_placement_each() {
    let package = package();
    let bytes = serde_json::to_vec(&package).unwrap();
    let source = PreparedProtocolSource::prepare(serde_json::from_slice(&bytes).unwrap()).unwrap();
    let expanded = source.expand("block-storage-entrance").unwrap();
    for kind in [
        "machine/usb/control",
        "machine/usb/endpoint-read",
        "machine/usb/endpoint-write",
    ] {
        assert_eq!(
            expanded
                .expanded
                .gears
                .iter()
                .filter(|gear| gear.kind_id.as_str() == kind)
                .count(),
            1
        );
    }
    assert!(
        !expanded
            .expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == "storage/block-read")
    );
    assert_eq!(expanded.front.inputs().len(), 1);
    assert_eq!(expanded.front.outputs().len(), 1);
    assert!(source.expand("bot-storage-provider").is_ok());
    let mut stale = package;
    stale.backs[0].checked_plot_id = "storage/stale-back".into();
    assert!(PreparedProtocolSource::prepare(stale).is_err());
}
fn program(source: &PreparedProtocolSource, name: &str) -> PortableExpressionProgram {
    let expanded = source.expand(name).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
#[test]
fn source_cbw_is_fixed_and_verified_before_class_effects() {
    let package = package();
    let source_text = package.source.clone();
    let source = PreparedProtocolSource::prepare(package).unwrap();
    let initialized = program(&source, "bot-initialize");
    let contract =
        conduitos::usb_base::endpoint_write_contract::EndpointWriteContract::prepare().unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        panic!()
    };
    let command = |bytes: &[u8]| {
        StructuredInfoValue::record(
            contract.request_type().clone(),
            vec![
                StructuredFieldValue::new(
                    "wire",
                    StructuredInfoValue::leaf(fields[0].value_type().clone(), bytes.to_vec())
                        .unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    };
    let hex = source_text
        .split("bytes/literal(\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let wire = conduit_data::bytes_literal::decode_hex(hex).unwrap();
    assert_eq!(wire.len(), 31);
    for altered in [false, true] {
        let mut wire = wire.clone();
        if altered {
            wire[4] = 2;
        }
        let encoded = initialized.evaluate(&command(&wire)).unwrap();
        let value = validate_canonical_structured_value(&encoded).unwrap();
        assert_eq!(
            value
                .record_field("terminal")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            &[u8::from(altered)]
        );
    }
}
fn fields(ty: &StructuredInfoType) -> &[StructuredFieldType] {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
}
fn record(
    ty: &StructuredInfoType,
    mut field: impl FnMut(&StructuredFieldType) -> StructuredInfoValue,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        fields(ty)
            .iter()
            .map(|definition| {
                StructuredFieldValue::new(definition.name(), field(definition)).unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn scalar(ty: &StructuredInfoType, value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), value.to_le_bytes().to_vec()).unwrap()
}
fn variant(
    ty: &StructuredInfoType,
    tag: &str,
    make: impl FnOnce(&StructuredInfoType) -> StructuredInfoValue,
) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    let payload = cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type();
    StructuredInfoValue::variant(ty.clone(), tag, make(payload)).unwrap()
}
#[test]
fn unsupported_geometry_and_lba_select_terminal_before_any_usb_route() {
    let package = package();
    let source = PreparedProtocolSource::prepare(package).unwrap();
    let pair = &source
        .checked
        .native_types
        .iter()
        .find(|ty| ty.name == "BotStoragePair")
        .unwrap()
        .value_type;
    let state_type = fields(pair)[0].value_type();
    let event_type = fields(pair)[1].value_type();
    let cbw = conduit_data::bytes_literal::decode_hex(
        "55534243010000000002000080000a28000000000000000100000000000000",
    )
    .unwrap();
    let request = |ty: &StructuredInfoType, size, count, lba| {
        record(ty, |field| {
            scalar(
                field.value_type(),
                match field.name() {
                    "block_size" => size,
                    "count" => count,
                    "lba" => lba,
                    _ => unreachable!(),
                },
            )
        })
    };
    let state = record(state_type, |field| match field.name() {
        "command" => record(field.value_type(), |wire| {
            StructuredInfoValue::leaf(wire.value_type().clone(), cbw.clone()).unwrap()
        }),
        "request" => request(field.value_type(), 512, 1, 0),
        "data" => variant(field.value_type(), "unsupported", |unit| {
            StructuredInfoValue::leaf(unit.clone(), vec![]).unwrap()
        }),
        "terminal" => StructuredInfoValue::leaf(field.value_type().clone(), vec![0]).unwrap(),
        _ => scalar(field.value_type(), 0),
    });
    let accept = program(&source, "bot-storage-accept");
    let advance = program(&source, "bot-storage-advance");
    let route = program(&source, "bot-storage-route");
    let emit = program(&source, "bot-storage-emission");
    for (size, count, lba) in [(512, 1, 0), (512, 1, 1), (512, 2, 0), (1024, 1, 0)] {
        let event = variant(event_type, "command", |ty| request(ty, size, count, lba));
        let pair = record(pair, |field| {
            if field.name() == "0" {
                state.clone()
            } else {
                event.clone()
            }
        });
        let transition = accept.evaluate(&pair.canonical_bytes().unwrap()).unwrap();
        let valid = size == 512 && count == 1 && lba == 0;
        assert_eq!(
            validate_canonical_structured_value(&transition)
                .unwrap()
                .record_field("accepted")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            &[u8::from(valid)]
        );
        let next = advance.evaluate(&transition).unwrap();
        let routed = route.evaluate(&next).unwrap();
        assert_eq!(
            validate_canonical_structured_value(&routed)
                .unwrap()
                .variant_tag()
                .unwrap(),
            if valid { "control" } else { "stopped" }
        );
        if !valid {
            let emission = emit.evaluate(&transition).unwrap();
            let value = validate_canonical_structured_value(&emission).unwrap();
            let result = value.variant_payload("emit").unwrap().unwrap();
            assert_eq!(result.variant_tag().unwrap(), "unsupported");
        }
    }
}
