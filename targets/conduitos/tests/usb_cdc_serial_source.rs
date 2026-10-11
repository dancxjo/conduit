//! Checked Source/Back eligibility only. No admitted CDC serial product claim.
use conduit_core::*;
use conduit_plot::{CanonicalBackCatalog, PortableExpressionProgram};
use conduitos::protocol_source::*;
use conduitos::serial_base::contract as serial_contract;

fn package() -> ProtocolSourcePackage {
    let (header, body) = include_str!("../plots/usb/cdc-serial-back.conduit")
        .split_once("\n\n")
        .unwrap();
    let value = |name: &str, maximum_bytes| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes,
    };
    let mut package = ProtocolSourcePackage::compile(
        format!(
            "{header}\n{}\n{}\n{}\n{}\n{body}\n{}",
            include_str!("../../../semantics/serial/duplex-bytes-types.conduit"),
            include_str!("../plots/usb/cdc-serial-types.conduit"),
            include_str!("../plots/usb/discovery-types.conduit"),
            include_str!("../plots/usb/cdc-serial-profile.conduit"),
            include_str!("../plots/usb/cdc-serial-producer.conduit")
        ),
        &[
            ProtocolSpecializationRequest::Singleton {
                value: value("CdcSerialEvent", 4096),
            },
            ProtocolSpecializationRequest::Concat {
                value: value("CdcSerialEvent", 4096),
            },
            ProtocolSpecializationRequest::SeededUntil {
                value: value("CdcSerialState", 4096),
            },
            ProtocolSpecializationRequest::FeedbackZip {
                left: value("CdcSerialState", 4096),
                right: value("CdcSerialEvent", 4096),
            },
            ProtocolSpecializationRequest::Zip {
                left: value("UsbControlResult", 4096),
                right: value("CdcAcmDescriptorSupported", 256),
            },
            ProtocolSpecializationRequest::Merge {
                value: value("CdcSerialEvent", 4096),
            },
        ],
    )
    .unwrap();
    let source = PreparedProtocolSource::prepare(package.clone()).unwrap();
    let plot = source
        .checked
        .plots
        .iter()
        .find(|plot| plot.name == "cdc-serial-provider")
        .unwrap();
    package.backs.push(ProtocolBackSelection {
        kind_id: kind_id("serial/duplex-bytes"),
        plot_name: plot.name.clone(),
        source_document_id: source.checked.source_document_id.clone(),
        checked_plot_id: plot.checked_plot_id.clone(),
    });
    package
}
fn prepared() -> PreparedProtocolSource {
    PreparedProtocolSource::prepare(package()).unwrap()
}
fn program(source: &PreparedProtocolSource, name: &str) -> PortableExpressionProgram {
    let expanded = source.expand(name).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn eval(program: &PortableExpressionProgram, value: &StructuredInfoValue) -> StructuredInfoValue {
    StructuredInfoValue::from_canonical_bytes(
        &program.evaluate(&value.canonical_bytes().unwrap()).unwrap(),
    )
    .unwrap()
}
fn fields(ty: &StructuredInfoType) -> &[StructuredFieldType] {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    fields
}
fn field<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    fields(ty)
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
fn payload<'a>(ty: &'a StructuredInfoType, tag: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!()
    };
    cases
        .iter()
        .find(|c| c.tag() == tag)
        .unwrap()
        .payload_type()
}
fn leaf(ty: &StructuredInfoType, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), bytes.to_vec()).unwrap()
}
fn record(
    ty: &StructuredInfoType,
    values: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        values
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
}
fn request(ty: &StructuredInfoType, tag: &str, bytes: &[u8]) -> StructuredInfoValue {
    let inner = payload(ty, tag);
    let name = fields(inner)[0].name();
    StructuredInfoValue::variant(
        ty.clone(),
        tag,
        record(inner, vec![(name, leaf(field(inner, name), bytes))]),
    )
    .unwrap()
}
fn member<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!()
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn scalar(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!()
    };
    bytes
}
fn pair(
    ty: &StructuredInfoType,
    left: &StructuredInfoValue,
    right: &StructuredInfoValue,
) -> StructuredInfoValue {
    record(
        ty,
        fields(ty)
            .iter()
            .zip([left, right])
            .map(|(f, v)| (f.name(), v.clone()))
            .collect(),
    )
}
#[test]
fn exact_serial_fore_is_a_checked_cdc_plot_back_and_mismatched_bounds_refuse() {
    let source = prepared();
    let contract = serial_contract::SerialDuplexContract::prepare().unwrap();
    let mut backs = CanonicalBackCatalog::new();
    backs
        .insert(contract.kind(), &source.checked, "cdc-serial-provider")
        .unwrap();
    let mut wrong = contract.kind().clone();
    let KindSemanticLaw::ValueContracts(contracts) = &mut wrong.semantic_laws[0] else {
        panic!()
    };
    contracts[0].contract.maximum_bytes -= 1;
    assert!(
        CanonicalBackCatalog::new()
            .insert(&wrong, &source.checked, "cdc-serial-provider")
            .is_err()
    );
}
#[test]
fn selected_serial_plot_back_expands_to_one_control_read_and_write_placement() {
    let source = prepared();
    let expanded = source.expand("cdc-serial-duplex-demonstration").unwrap();
    for name in [
        "machine/usb/control",
        "machine/usb/endpoint-read",
        "machine/usb/endpoint-write",
    ] {
        assert_eq!(
            expanded
                .expanded
                .gears
                .iter()
                .filter(|gear| gear.kind_id.as_str() == name)
                .count(),
            1
        );
    }
}
#[test]
fn semantic_commands_select_exact_octet_requests_without_synthetic_repeat_or_utf8() {
    let source = prepared();
    let init = program(&source, "cdc-serial-producer-start");
    let emit = program(&source, "cdc-serial-producer-request");
    let tx = request(&init.input_type, "transmit", &[255, 0, 128, 10]);
    let mut state = eval(&init, &tx);
    let next = program(&source, "cdc-serial-producer-next");
    assert_eq!(eval(&emit, &state), tx);
    // The generic producer emits one new explicit request only after a result.
    for ordinal in 0..32u64 {
        let event_ty = fields(&next.input_type)[1].value_type();
        let result_ty = payload(event_ty, "serial");
        let tag = if ordinal % 2 == 0 {
            "transmitted"
        } else {
            "received"
        };
        let info = payload(result_ty, tag);
        let mut values = vec![
            ("actual", leaf(field(info, "actual"), &4u64.to_le_bytes())),
            (
                "ordinal",
                leaf(field(info, "ordinal"), &ordinal.to_le_bytes()),
            ),
            (
                "requested",
                leaf(
                    field(info, "requested"),
                    &(if ordinal % 2 == 0 { 4u64 } else { 64 }).to_le_bytes(),
                ),
            ),
        ];
        if ordinal % 2 == 1 {
            values.push(("bytes", leaf(field(info, "bytes"), &[255, 0, 128, 10])));
            values.push(("short", leaf(field(info, "short"), &[1])));
        }
        let result =
            StructuredInfoValue::variant(result_ty.clone(), tag, record(info, values)).unwrap();
        let event = StructuredInfoValue::variant(event_ty.clone(), "serial", result).unwrap();
        state = eval(&next, &pair(&next.input_type, &state, &event));
        assert_eq!(
            scalar(member(&state, "operations")),
            (ordinal + 1).to_le_bytes()
        );
        assert_eq!(
            scalar(member(&state, "terminal")),
            &[u8::from(ordinal == 31)]
        );
    }
}

fn state(ty: &StructuredInfoType, phase: u64, operations: u64) -> StructuredInfoValue {
    record(
        ty,
        vec![
            (
                "request",
                request(field(ty, "request"), "transmit", &[255, 0, 128]),
            ),
            ("phase", leaf(field(ty, "phase"), &phase.to_le_bytes())),
            (
                "operations",
                leaf(field(ty, "operations"), &operations.to_le_bytes()),
            ),
            ("reads", leaf(field(ty, "reads"), &0u64.to_le_bytes())),
            ("terminal", leaf(field(ty, "terminal"), &[0])),
        ],
    )
}
#[test]
fn ordered_request_closure_stops_zero_one_or_thirty_two_without_semantic_result_or_effect() {
    let source = prepared();
    let accept = program(&source, "cdc-serial-accept");
    let advance = program(&source, "cdc-serial-advance");
    let emission = program(&source, "cdc-serial-emission");
    let route = program(&source, "cdc-serial-route");
    let event_ty = fields(&accept.input_type)[1].value_type();
    let closed = StructuredInfoValue::variant(
        event_ty.clone(),
        "closed",
        leaf(payload(event_ty, "closed"), &[]),
    )
    .unwrap();
    for operations in [0u64, 1, 32] {
        let current = state(
            fields(&accept.input_type)[0].value_type(),
            if operations == 0 { 0 } else { 8 },
            operations,
        );
        let transition = eval(&accept, &pair(&accept.input_type, &current, &closed));
        assert_eq!(scalar(member(&transition, "accepted")), &[1]);
        let next = eval(&advance, &transition);
        assert_eq!(
            scalar(member(&next, "operations")),
            operations.to_le_bytes()
        );
        assert_eq!(scalar(member(&next, "terminal")), &[1]);
        assert!(
            matches!(eval(&route,&next).shape(),StructuredInfoValueShape::Variant{tag,..} if tag=="stopped")
        );
        assert!(
            matches!(eval(&emission,&transition).shape(),StructuredInfoValueShape::Variant{tag,..} if tag=="absent")
        );
    }
    // The closure event belongs to the request-readiness gate. It cannot be
    // accepted during a physical transfer; concat/feedback must first retain
    // it until that transfer's result returns the ready state.
    let pending = state(fields(&accept.input_type)[0].value_type(), 6, 1);
    let transition = eval(&accept, &pair(&accept.input_type, &pending, &closed));
    assert_eq!(scalar(member(&transition, "accepted")), &[0]);
}
#[test]
fn thirty_third_request_is_typed_refusal_before_another_usb_submission() {
    let source = prepared();
    let accept = program(&source, "cdc-serial-accept");
    let advance = program(&source, "cdc-serial-advance");
    let emission = program(&source, "cdc-serial-emission");
    let event_ty = fields(&accept.input_type)[1].value_type();
    let command = StructuredInfoValue::variant(
        event_ty.clone(),
        "command",
        request(payload(event_ty, "command"), "transmit", &[255]),
    )
    .unwrap();
    let current = state(fields(&accept.input_type)[0].value_type(), 8, 32);
    let transition = eval(&accept, &pair(&accept.input_type, &current, &command));
    assert_eq!(scalar(member(&transition, "accepted")), &[0]);
    assert_eq!(
        scalar(member(&eval(&advance, &transition), "terminal")),
        &[0]
    );
    let emitted = eval(&emission, &transition);
    assert!(
        matches!(emitted.shape(),StructuredInfoValueShape::Variant{tag,payload} if tag=="emit" && matches!(payload.shape(),StructuredInfoValueShape::Variant{tag,..} if tag=="unsupported"))
    );
}

#[test]
fn packaged_serial_back_roundtrip_selects_checked_source_and_refuses_stale_identity_or_fore() {
    let package = package();
    let encoded = serde_json::to_vec(&package).unwrap();
    let decoded: ProtocolSourcePackage = serde_json::from_slice(&encoded).unwrap();
    let source = PreparedProtocolSource::prepare(decoded.clone()).unwrap();
    let expanded = source.expand("cdc-serial-duplex-demonstration").unwrap();
    assert!(
        !expanded
            .expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == "serial/duplex-bytes")
    );
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
    let mut stale = decoded.clone();
    stale.backs[0].checked_plot_id = CheckedPlotId::from("stale/cdc-back@1");
    assert!(PreparedProtocolSource::prepare(stale).is_err());
    let mut wrong_document = decoded.clone();
    wrong_document.backs[0].source_document_id = SourceDocumentId::from("stale/cdc-source@1");
    assert!(PreparedProtocolSource::prepare(wrong_document).is_err());
    let mut wrong_fore = decoded;
    wrong_fore.source = wrong_fore.source.replace(
        "request: SerialDuplexRequest...| <= 4096B >> result: SerialDuplexResult...| <= 4096B",
        "request: SerialDuplexRequest...| <= 4095B >> result: SerialDuplexResult...| <= 4096B",
    );
    let checked = PreparedProtocolSource::prepare({
        let mut fresh = wrong_fore.clone();
        fresh.backs.clear();
        fresh
    })
    .unwrap();
    wrong_fore.backs[0].source_document_id = checked.checked.source_document_id.clone();
    wrong_fore.backs[0].checked_plot_id = checked
        .checked
        .plots
        .iter()
        .find(|plot| plot.name == "cdc-serial-provider")
        .unwrap()
        .checked_plot_id
        .clone();
    assert!(PreparedProtocolSource::prepare(wrong_fore).is_err());
}

#[path = "usb_cdc_serial/mod.rs"]
mod kernel_execution;
