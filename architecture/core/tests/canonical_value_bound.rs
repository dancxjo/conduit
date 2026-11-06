use conduit_core::{
    kind_id, KindId, StructuredFieldType, StructuredFieldValue, StructuredInfoType as Type,
    StructuredInfoValue as Value, StructuredVariantCase,
};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;

fn bound(kind: &KindId) -> Option<usize> {
    match kind.as_str() {
        "value/u8" => Some(1),
        "value/bytes" => Some(3),
        "value/empty" => Some(0),
        _ => None,
    }
}
fn octet() -> Value {
    Value::leaf(Type::leaf(kind_id("value/u8")).unwrap(), vec![255]).unwrap()
}

#[test]
fn bounds_match_independent_maximal_canonical_values_for_every_shape() {
    let octet_type = octet().value_type().clone();
    let nominal = Type::nominal(kind_id("example/nominal"), octet_type.clone()).unwrap();
    let collection = Type::collection(octet_type.clone(), Some(3)).unwrap();
    let sequence = Type::bounded_sequence(octet_type.clone(), 0, 3).unwrap();
    let record = Type::record(
        kind_id("example/record"),
        vec![StructuredFieldType::new("é", sequence.clone()).unwrap()],
    )
    .unwrap();
    let unit = Type::leaf(kind_id("value/empty")).unwrap();
    let variant = Type::variant(
        kind_id("example/variant"),
        vec![
            StructuredVariantCase::new("a", record.clone()).unwrap(),
            StructuredVariantCase::new("a-much-longer-tag", unit.clone()).unwrap(),
        ],
    )
    .unwrap();
    let record_value = || {
        Value::record(
            record.clone(),
            vec![StructuredFieldValue::new(
                "é",
                Value::sequence(sequence.clone(), vec![octet(); 3]).unwrap(),
            )
            .unwrap()],
        )
        .unwrap()
    };
    let values = vec![
        octet(),
        Value::nominal(nominal, octet()).unwrap(),
        Value::collection(collection, vec![octet(); 3]).unwrap(),
        Value::sequence(sequence.clone(), vec![octet(); 3]).unwrap(),
        record_value(),
        Value::variant(variant.clone(), "a", record_value()).unwrap(),
        Value::leaf(Type::leaf(kind_id("value/bytes")).unwrap(), vec![255; 3]).unwrap(),
    ];
    for value in values {
        let encoded = value.canonical_bytes().unwrap();
        let (maximum, cost) =
            probe::observe(|| value.value_type().canonical_value_upper_bound(bound));
        assert_eq!(maximum, Some(encoded.len()));
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    }
    let alternate = Value::variant(
        variant,
        "a-much-longer-tag",
        Value::leaf(unit, vec![]).unwrap(),
    )
    .unwrap();
    assert!(
        alternate.canonical_bytes().unwrap().len()
            < alternate
                .value_type()
                .canonical_value_upper_bound(bound)
                .unwrap()
    );
    for length in 0..=3 {
        let value = Value::sequence(sequence.clone(), vec![octet(); length]).unwrap();
        assert!(
            value.canonical_bytes().unwrap().len()
                <= sequence.canonical_value_upper_bound(bound).unwrap()
        );
    }
}

#[test]
fn unknown_or_excessive_leaf_contracts_refuse_without_allocation() {
    let value_type = Type::leaf(kind_id("value/bytes")).unwrap();
    for limit in [None, Some(usize::MAX), Some(262_144)] {
        let (result, cost) = probe::observe(|| value_type.canonical_value_upper_bound(|_| limit));
        assert_eq!(result, None);
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    }
    let huge = Type::bounded_sequence(value_type, 0, 1024).unwrap();
    assert_eq!(huge.canonical_value_upper_bound(|_| Some(1024)), None);
}
