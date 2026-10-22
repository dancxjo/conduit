use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape as T,
    StructuredInfoValue as V, StructuredInfoValueShape as S,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

fn program() -> PortableExpressionProgram {
    let syntax = parse_syntax_document(include_str!("../../device-protocols/tm1637.conduit"));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "tm1637-packets", &ProfileCatalog::new())
            .unwrap()
            .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("Source program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn field<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let T::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
fn input(program: &PortableExpressionProgram, brightness: u8, enabled: bool) -> Vec<u8> {
    let ty = &program.input_type;
    let digits = field(ty, "registers");
    let T::Collection { element, .. } = digits.shape() else {
        panic!("octets")
    };
    let digits = V::collection(
        digits.clone(),
        [0x7d, 0x87, 0x7d, 0x71, 0x80, 0]
            .into_iter()
            .map(|b| V::leaf(element.clone(), vec![b]).unwrap())
            .collect(),
    )
    .unwrap();
    V::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new("registers", digits).unwrap(),
            StructuredFieldValue::new(
                "brightness",
                V::leaf(field(ty, "brightness").clone(), vec![brightness]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "enabled",
                V::leaf(field(ty, "enabled").clone(), vec![u8::from(enabled)]).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn octets(record: &V, name: &str) -> Vec<u8> {
    let S::Record(fields) = record.shape() else {
        panic!("record")
    };
    let S::Collection(items) = fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value()
        .shape()
    else {
        panic!("octets")
    };
    items
        .iter()
        .map(|v| {
            let S::Leaf(bytes) = v.shape() else {
                panic!("octet")
            };
            bytes[0]
        })
        .collect()
}
fn packets(record: &V) -> Vec<Vec<u8>> {
    let S::Record(fields) = record.shape() else {
        panic!("packets")
    };
    let S::Collection(packets) = fields
        .iter()
        .find(|f| f.name() == "packets")
        .unwrap()
        .value()
        .shape()
    else {
        panic!("packet order")
    };
    packets
        .iter()
        .map(|packet| {
            let S::Record(fields) = packet.shape() else {
                panic!("packet")
            };
            let S::Leaf(length) = fields
                .iter()
                .find(|f| f.name() == "length")
                .unwrap()
                .value()
                .shape()
            else {
                panic!("length")
            };
            let mut bytes = octets(packet, "wire");
            assert!(bytes[length[0] as usize..].iter().all(|byte| *byte == 0));
            bytes.truncate(length[0] as usize);
            bytes
        })
        .collect()
}
#[test]
fn source_owns_command_order_display_control_and_brightness_refusal() {
    let program = program();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for enabled in [false, true] {
        for brightness in 0..=7 {
            let result = prepared
                .evaluate(&input(&program, brightness, enabled))
                .unwrap()
                .to_vec();
            let result = V::from_canonical_bytes(&result).unwrap();
            let S::Variant { tag, payload } = result.shape() else {
                panic!("result")
            };
            assert_eq!(tag, "packets");
            assert_eq!(
                packets(payload),
                [
                    vec![0x40],
                    vec![0xc0, 0x7d, 0x87, 0x7d, 0x71, 0x80, 0],
                    vec![if enabled { 0x88 } else { 0x80 } | brightness]
                ]
            );
        }
    }
    for brightness in [8, 255] {
        let result = V::from_canonical_bytes(
            prepared
                .evaluate(&input(&program, brightness, true))
                .unwrap(),
        )
        .unwrap();
        let S::Variant { tag, .. } = result.shape() else {
            panic!("result")
        };
        assert_eq!(tag, "invalid-brightness");
    }
}
