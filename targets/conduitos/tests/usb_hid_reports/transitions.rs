use super::common::program_from;
use conduit_core::*;
use conduit_plot::PreparedPortableExpressionEvaluator;

fn field(schema: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
        panic!("record");
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}

fn report(schema: StructuredInfoType, modifiers: u8, keys: [u8; 6]) -> StructuredInfoValue {
    let octet = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let keys_type = field(&schema, "keys");
    StructuredInfoValue::record(
        schema,
        vec![
            StructuredFieldValue::new(
                "modifiers",
                StructuredInfoValue::leaf(octet.clone(), vec![modifiers]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "keys",
                StructuredInfoValue::collection(
                    keys_type,
                    keys.into_iter()
                        .map(|k| StructuredInfoValue::leaf(octet.clone(), vec![k]).unwrap())
                        .collect(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn input(schema: &StructuredInfoType, previous: (u8, [u8; 6]), current: (u8, [u8; 6])) -> Vec<u8> {
    StructuredInfoValue::record(
        schema.clone(),
        vec![
            StructuredFieldValue::new(
                "previous",
                report(field(schema, "previous"), previous.0, previous.1),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "current",
                report(field(schema, "current"), current.0, current.1),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

fn expected(previous: (u8, [u8; 6]), current: (u8, [u8; 6])) -> Vec<(u8, bool, u8)> {
    let mut events = Vec::new();
    for bit in 0..8 {
        let mask = 1 << bit;
        if (previous.0 ^ current.0) & mask != 0 {
            events.push((224 + bit, current.0 & mask != 0, current.0));
        }
    }
    for key in previous.1 {
        if key != 0 && !current.1.contains(&key) {
            events.push((key, false, current.0));
        }
    }
    for key in current.1 {
        if key != 0 && !previous.1.contains(&key) {
            events.push((key, true, current.0));
        }
    }
    events
}

fn observed(bytes: &[u8]) -> Vec<(u8, bool, u8)> {
    let root = validate_canonical_structured_value(bytes).unwrap();
    let batch = root.record_field("slots").unwrap().unwrap();
    assert_eq!(batch.collection_length().unwrap(), 20);
    let mut events = Vec::new();
    for index in 0..20 {
        let slot = batch.collection_index(index).unwrap().unwrap();
        if let Some(event) = slot.variant_payload("changed").unwrap() {
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
            events.push((octet("usage"), pressed, octet("modifiers")));
        } else {
            assert!(slot.variant_payload("unchanged").unwrap().is_some());
        }
    }
    events
}

#[test]
fn source_transitions_preserve_modifier_release_press_order_and_twenty_bound() {
    let source = format!(
        "{}\n{}",
        super::common::SOURCE,
        include_str!("../../plots/usb/hid-keyboard-state.conduit")
    );
    let program = program_from(&source, "usb-hid-keyboard-transitions");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let mut cases = vec![
        ((0, [4, 5, 6, 7, 8, 9]), (255, [10, 11, 12, 13, 14, 15])),
        ((255, [10, 11, 12, 13, 14, 15]), (0, [4, 5, 6, 7, 8, 9])),
        ((42, [0, 0, 4, 7, 254, 255]), (42, [0, 0, 4, 7, 254, 255])),
        ((127, [0; 6]), (128, [0, 0, 0, 0, 4, 255])),
        ((128, [0, 0, 0, 0, 4, 255]), (127, [0; 6])),
        ((0, [0, 4, 5, 6, 7, 8]), (1, [0, 5, 6, 7, 8, 255])),
    ];
    for bit in 0..8 {
        cases.push(((0, [0; 6]), (1 << bit, [0; 6])));
        cases.push(((1 << bit, [0; 6]), (0, [0; 6])));
    }
    for (previous, current) in cases {
        let bytes = input(&program.input_type, previous, current);
        let ordinary = program.evaluate(&bytes).unwrap();
        let want = expected(previous, current);
        let allocations = crate::allocation::allocations(|| {
            assert_eq!(prepared.evaluate(&bytes).unwrap(), ordinary);
        });
        assert_eq!(allocations, 0);
        assert_eq!(observed(&ordinary), want);
    }
}
