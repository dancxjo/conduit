//! Pure checked frame adapter; no USB/network authority or native device proof.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
use conduitos::{
    ethernet_frame::contract::FRAME_TYPES,
    protocol_source::{PreparedProtocolSource, ProtocolSourcePackage},
};
fn prepared() -> PreparedProtocolSource {
    let adapter = include_str!("../plots/usb/cdc-ecm-frames.conduit");
    let imports = adapter
        .lines()
        .filter(|line| line.starts_with("with machine/usb/"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = adapter
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    PreparedProtocolSource::prepare(
        ProtocolSourcePackage::compile(format!("{imports}\n{FRAME_TYPES}\n{body}\n"), &[]).unwrap(),
    )
    .unwrap()
}
fn program(prepared: &PreparedProtocolSource) -> PortableExpressionProgram {
    let expanded = prepared.expand("cdc-ecm-completed-frame").unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0]
        .configuration
        .iter()
        .find(|entry| entry.key == "program")
        .unwrap()
        .value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn case<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|case| case.tag() == name)
        .unwrap()
        .payload_type()
}
fn field<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
}
fn observed(program: &PortableExpressionProgram, wire: &[u8], actual: u64, short: bool) -> Vec<u8> {
    let ty = case(&program.input_type, "completed");
    let record = StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new(
                "ordinal",
                StructuredInfoValue::leaf(
                    field(ty, "ordinal").clone(),
                    0u64.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "actual",
                StructuredInfoValue::leaf(
                    field(ty, "actual").clone(),
                    actual.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "short",
                StructuredInfoValue::leaf(field(ty, "short").clone(), vec![u8::from(short)])
                    .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "wire",
                StructuredInfoValue::leaf(field(ty, "wire").clone(), wire.to_vec()).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoValue::variant(program.input_type.clone(), "completed", record)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
fn frame(length: usize) -> Vec<u8> {
    let mut bytes: Vec<_> = (0..length).map(|index| (index % 251) as u8).collect();
    if length >= 14 {
        bytes[..6].fill(0xff);
        bytes[6..12].copy_from_slice(&[2, 0, 0, 0, 0, 1]);
        bytes[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
    }
    bytes
}
fn check(
    prepared: &PreparedProtocolSource,
    program: &PortableExpressionProgram,
    evaluator: &mut PreparedPortableExpressionEvaluator,
    input: &[u8],
    expected: &str,
    wire: &[u8],
) {
    let bytes = evaluator.evaluate(input).unwrap();
    assert_eq!(bytes, program.evaluate(input).unwrap());
    let result = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = result.shape() else {
        panic!("gate")
    };
    assert_eq!(tag, expected);
    let native = prepared
        .checked
        .native_types
        .iter()
        .find(|ty| ty.name == "CdcEcmReceiveGate")
        .unwrap();
    conduit_plot::rust_binding::validate_native_contracts(&result, &native.value_contracts)
        .unwrap();
    if expected == "ready" {
        let StructuredInfoValueShape::Variant { tag, payload } = payload.shape() else {
            panic!("request")
        };
        assert_eq!(tag, "deliver-receive");
        let StructuredInfoValueShape::Record(fields) = payload.shape() else {
            panic!("frame")
        };
        let StructuredInfoValueShape::Leaf(bytes) = fields[0].value().shape() else {
            panic!("wire")
        };
        assert_eq!(bytes, wire);
    }
}
#[test]
fn checked_ecm_adapter_expands_and_preserves_actual_wire_only_inside_bound() {
    let prepared = prepared();
    prepared.expand("cdc-ecm-frame-request").unwrap();
    let program = program(&prepared);
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for length in [13, 14, 1514, 1515, 2048] {
        let wire = frame(length);
        // Legal MTU is short relative to the one-byte sentinel request.
        let input = observed(&program, &wire, length as u64, length < 1515);
        check(
            &prepared,
            &program,
            &mut evaluator,
            &input,
            if (14..=1514).contains(&length) {
                "ready"
            } else {
                "malformed"
            },
            &wire,
        );
    }
}
#[test]
fn metadata_header_and_transport_faults_refuse_without_reinterpreting_wire() {
    let prepared = prepared();
    let program = program(&prepared);
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let wire = frame(14);
    for (actual, short) in [(13, true), (15, true), (14, false), (u64::MAX, true)] {
        let input = observed(&program, &wire, actual, short);
        check(
            &prepared,
            &program,
            &mut evaluator,
            &input,
            "malformed",
            &wire,
        );
    }
    let mut faults = Vec::new();
    let mut multicast = wire.clone();
    multicast[6] |= 1;
    faults.push(multicast);
    let mut zero = wire.clone();
    zero[6..12].fill(0);
    faults.push(zero);
    for ether_type in [0x05ffu16, 0x8100, 0x88a8] {
        let mut bytes = wire.clone();
        bytes[12..14].copy_from_slice(&ether_type.to_be_bytes());
        faults.push(bytes);
    }
    for wire in faults {
        let input = observed(&program, &wire, 14, true);
        check(
            &prepared,
            &program,
            &mut evaluator,
            &input,
            "malformed",
            &wire,
        );
    }
    for tag in ["stalled", "provider-lost", "unsupported", "timeout"] {
        let unit =
            StructuredInfoValue::leaf(case(&program.input_type, tag).clone(), vec![]).unwrap();
        let input = StructuredInfoValue::variant(program.input_type.clone(), tag, unit)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        check(&prepared, &program, &mut evaluator, &input, tag, &[]);
    }
}
#[test]
fn output_constructor_native_validation_still_rejects_oversized_bytes() {
    let prepared = prepared();
    let program = program(&prepared);
    let request = case(&program.output_type, "ready");
    let ty = case(request, "deliver-receive");
    let frame = StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new(
                "wire",
                StructuredInfoValue::leaf(field(ty, "wire").clone(), vec![0; 1515]).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let request = StructuredInfoValue::variant(request.clone(), "deliver-receive", frame).unwrap();
    let result =
        StructuredInfoValue::variant(program.output_type.clone(), "ready", request).unwrap();
    let native = prepared
        .checked
        .native_types
        .iter()
        .find(|ty| ty.name == "CdcEcmReceiveGate")
        .unwrap();
    assert!(
        conduit_plot::rust_binding::validate_native_contracts(&result, &native.value_contracts)
            .is_err()
    );
}
