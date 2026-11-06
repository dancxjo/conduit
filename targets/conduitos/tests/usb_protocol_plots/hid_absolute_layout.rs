//! Bounded HID descriptor-prefix meaning, independent of a USB transport.
use conduit_core::*;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
const PREFIX: &[u8] = &[
    0x05, 0x0d, 0x09, 0x04, 0xa1, 0x01, 0x85, 0x01, 0x09, 0x22, 0xa1, 0x02, 0x09, 0x42, 0x15, 0x00,
    0x25, 0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0x95, 0x07, 0x81, 0x01, 0x75, 0x08, 0x09, 0x51,
    0x95, 0x01, 0x81, 0x02, 0x05, 0x01, 0x26, 0x00, 0x04, 0x75, 0x10, 0x55, 0x0e, 0x65, 0x11, 0x09,
    0x30, 0x35, 0x00, 0x46, 0x79, 0x08, 0x81, 0x02, 0x26, 0x58, 0x02, 0x46, 0x4c, 0x05, 0x09, 0x31,
    0x81, 0x02,
];
fn field(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn input(ty: &StructuredInfoType, wire: &[u8]) -> Vec<u8> {
    let octet = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new(
                "wire",
                StructuredInfoValue::sequence(
                    field(ty, "wire"),
                    wire.iter()
                        .map(|b| StructuredInfoValue::leaf(octet.clone(), vec![*b]).unwrap())
                        .collect(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "actual",
                StructuredInfoValue::leaf(
                    field(ty, "actual"),
                    (wire.len() as u64).to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
struct Parser {
    programs: [PortableExpressionProgram; 5],
    evaluators: [PreparedPortableExpressionEvaluator; 5],
    state: Vec<u8>,
}
impl Parser {
    fn new() -> Self {
        let source = conduitos::protocol_source::PreparedProtocolSource::prepare(
            conduitos::protocol_source::usb_hid_absolute_descriptor_package().unwrap(),
        )
        .unwrap();
        let programs = ["start", "load", "classify", "step", "finish"].map(|name| {
            let expanded = source
                .expand(&format!("usb-hid-absolute-layout-{name}"))
                .unwrap()
                .expanded;
            assert_eq!(expanded.gears.len(), 1);
            let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value
            else {
                panic!("pure Source program");
            };
            PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
        });
        for i in 0..4 {
            assert_eq!(
                programs[i].output_type,
                programs[i + 1].input_type,
                "stage {i}"
            );
        }
        let evaluators = programs
            .each_ref()
            .map(|p| PreparedPortableExpressionEvaluator::new(p).unwrap());
        Self {
            programs,
            evaluators,
            state: Vec::with_capacity(8192),
        }
    }
    fn run(&mut self, wire: &[u8]) -> Vec<u8> {
        let bytes = input(&self.programs[0].input_type, wire);
        self.state.clear();
        self.state
            .extend_from_slice(self.evaluators[0].evaluate(&bytes).unwrap());
        let [_, load, classify, step, finish] = &mut self.evaluators;
        let allocations = super::allocation::allocations(|| {
            for index in 0..32 {
                let loaded = load.evaluate(&self.state).unwrap_or_else(|e| {
                    panic!(
                        "load {index}, bytes={}, state_valid={:?}, error={e:?}",
                        self.state.len(),
                        validate_canonical_structured_value(&self.state).map(|_| ())
                    )
                });
                let classified = classify.evaluate(loaded).unwrap();
                let advanced = step.evaluate(classified).unwrap();
                self.state.clear();
                self.state.extend_from_slice(advanced);
            }
            finish.evaluate(&self.state).unwrap();
        });
        assert_eq!(allocations, 0);
        finish.evaluate(&self.state).unwrap().to_vec()
    }
}
#[test]
fn absolute_contact_layout_comes_from_usage_items_and_logical_ranges() {
    let mut parser = Parser::new();
    for (wire, maximum_x, maximum_y) in [(PREFIX.to_vec(), 1024_u16, 600_u16), {
        let mut changed = PREFIX.to_vec();
        changed[40] = 0x03;
        changed[57] = 0x40;
        changed[58] = 0x01;
        (changed, 768, 320)
    }] {
        let result = parser.run(&wire);
        let layout = validate_canonical_structured_value(&result)
            .unwrap()
            .variant_payload("layout")
            .unwrap()
            .unwrap();
        for (name, expected) in [("maximum-x", maximum_x), ("maximum-y", maximum_y)] {
            assert_eq!(
                layout
                    .record_field(name)
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u16")
                    .unwrap(),
                expected.to_le_bytes()
            );
        }
        for (name, expected) in [("tip-byte", 1_u64), ("x-byte", 3), ("y-byte", 5)] {
            assert_eq!(
                layout
                    .record_field(name)
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                expected.to_le_bytes()
            );
        }
        assert_eq!(
            layout
                .record_field("report-id")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u8")
                .unwrap(),
            [1]
        );
    }
}
#[test]
fn absolute_layout_refuses_relative_fields_missing_contact_and_truncated_items() {
    let mut parser = Parser::new();
    let mut relative = PREFIX.to_vec();
    relative[65] = 6;
    let mut wrong_usage = PREFIX.to_vec();
    wrong_usage[13] = 0x43;
    for (wire, tag) in [
        (relative, "unsupported"),
        (wrong_usage, "unsupported"),
        (PREFIX[..65].to_vec(), "short"),
        (vec![0xc0], "malformed"),
        (vec![0x05, 0x0d, 0xa4], "unsupported"),
        (vec![0; 64], "pressure"),
    ] {
        let result = parser.run(&wire);
        assert!(
            validate_canonical_structured_value(&result)
                .unwrap()
                .variant_payload(tag)
                .unwrap()
                .is_some(),
            "expected {tag}"
        );
    }
}
#[test]
fn complete_absolute_layout_plot_uses_bounded_source_steps() {
    use conduitos::protocol_source::PreparedProtocolSource;
    let source = PreparedProtocolSource::prepare(
        conduitos::protocol_source::usb_hid_absolute_descriptor_package().unwrap(),
    )
    .unwrap();
    let expanded = source.expand("usb-hid-absolute-layout").unwrap().expanded;
    assert_eq!(expanded.gears.len(), 9);
    for gear in &expanded.gears {
        if gear.kind_contract_revision.as_str() != conduit_plot::PURE_EXPRESSION_REVISION {
            continue;
        }
        let [parameter] = gear.configuration.as_slice() else {
            panic!("one pure program")
        };
        let ConfigurationValue::Text(encoded) = &parameter.value else {
            panic!("pure program bytes")
        };
        PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    }
}

#[path = "hid_absolute_layout/execution.rs"]
mod execution;
