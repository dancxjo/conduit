//! Lowered selectors preserve semantic Types, predicates and refusal behavior.
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn leaf() -> StructuredInfoType {
    StructuredInfoType::leaf("value/count".into()).unwrap()
}
fn value(n: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(leaf(), n.to_le_bytes().to_vec()).unwrap()
}
fn nominal(ty: StructuredInfoType, value: StructuredInfoValue) -> StructuredInfoValue {
    let nominal = StructuredInfoType::nominal("test/nominal@1".into(), ty).unwrap();
    StructuredInfoValue::nominal(nominal, value).unwrap()
}
fn compare(selector: StructuredSelector, inputs: Vec<StructuredInfoValue>) {
    let mut prepared = PreparedCanonicalStructuredSelector::prepare(&selector, 4096, 4096).unwrap();
    for value in inputs {
        let expected = selector.select(&value).and_then(|selected| match selected {
            StructuredSelection::Matched(value) => Ok(Some(value.canonical_bytes().unwrap())),
            StructuredSelection::Unmatched(UnmatchedVariantDisposition::Drop) => Ok(None),
            StructuredSelection::Unmatched(UnmatchedVariantDisposition::Refuse) => {
                Err(StructuredSelectorRefusal::UnmatchedVariant)
            }
        });
        let input = value.canonical_bytes().unwrap();
        assert_eq!(
            prepared.select(&input).map(|value| value.map(Vec::from)),
            expected
        );
    }
}
#[test]
fn lowered_field_index_variant_and_predicate_match_including_nominal_types() {
    let record = StructuredInfoType::record(
        "test/record@1".into(),
        vec![StructuredFieldType::new("n", leaf()).unwrap()],
    )
    .unwrap();
    let one = StructuredInfoValue::record(
        record.clone(),
        vec![StructuredFieldValue::new("n", value(1)).unwrap()],
    )
    .unwrap();
    let two = StructuredInfoValue::record(
        record.clone(),
        vec![StructuredFieldValue::new("n", value(2)).unwrap()],
    )
    .unwrap();
    compare(
        StructuredSelector::field(record.clone(), "n").unwrap(),
        vec![one.clone()],
    );
    let named = nominal(record.clone(), one.clone());
    compare(
        StructuredSelector::field(named.value_type().clone(), "n").unwrap(),
        vec![named],
    );
    let collection = StructuredInfoType::collection(leaf(), Some(2)).unwrap();
    let pair =
        StructuredInfoValue::collection(collection.clone(), vec![value(1), value(2)]).unwrap();
    let named = nominal(collection, pair);
    compare(
        StructuredSelector::index(named.value_type().clone(), 1).unwrap(),
        vec![named],
    );
    let variant = StructuredInfoType::variant(
        "test/variant@1".into(),
        vec![
            StructuredVariantCase::new("a", record.clone()).unwrap(),
            StructuredVariantCase::new("b", leaf()).unwrap(),
        ],
    )
    .unwrap();
    let a = StructuredInfoValue::variant(variant.clone(), "a", one.clone()).unwrap();
    let b = StructuredInfoValue::variant(variant.clone(), "b", value(2)).unwrap();
    for unmatched in [
        UnmatchedVariantDisposition::Drop,
        UnmatchedVariantDisposition::Refuse,
    ] {
        compare(
            StructuredSelector::variant(variant.clone(), "a", unmatched).unwrap(),
            vec![a.clone(), b.clone()],
        );
        let named_a = nominal(variant.clone(), a.clone());
        let named_b = nominal(variant.clone(), b.clone());
        compare(
            StructuredSelector::variant(named_a.value_type().clone(), "a", unmatched).unwrap(),
            vec![named_a, named_b],
        );
        for match_when_present in [false, true] {
            compare(
                StructuredSelector::record_field_values(
                    record.clone(),
                    "n",
                    vec![1u64.to_le_bytes().to_vec()],
                    match_when_present,
                    unmatched,
                )
                .unwrap(),
                vec![one.clone(), two.clone()],
            );
        }
    }
}
#[test]
fn evaluation_allocates_nothing_and_refuses_wrong_type_malformed_and_pressure() {
    let ty = StructuredInfoType::collection(leaf(), Some(2)).unwrap();
    let input = StructuredInfoValue::collection(ty.clone(), vec![value(1), value(2)])
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let expected = value(2).canonical_bytes().unwrap();
    let selector = StructuredSelector::index(ty, 1).unwrap();
    let mut prepared = PreparedCanonicalStructuredSelector::prepare(&selector, 4096, 4096).unwrap();
    let mut too_small = PreparedCanonicalStructuredSelector::prepare(&selector, 4096, 0).unwrap();
    let wrong = value(2).canonical_bytes().unwrap();
    let mut trailing = input.clone();
    trailing.push(0);
    let mut malformed = input.clone();
    *malformed.last_mut().unwrap() = 255;
    // A truncated leaf is invalid regardless of its declared primitive value.
    malformed.pop();
    let ((), memory) = allocation_probe::observe(|| {
        for _ in 0..8 {
            assert_eq!(prepared.select(&input).unwrap().unwrap(), expected);
        }
        for bytes in [&wrong, &trailing, &malformed] {
            assert_eq!(
                prepared.select(bytes),
                Err(StructuredSelectorRefusal::MalformedCheckedValue)
            );
        }
        assert_eq!(
            too_small.select(&input),
            Err(StructuredSelectorRefusal::CanonicalEncodingTooLarge)
        );
        assert_eq!(prepared.select(&input).unwrap().unwrap(), expected);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
