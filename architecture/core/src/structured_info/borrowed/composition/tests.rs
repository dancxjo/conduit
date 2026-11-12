use super::*;
use crate::{
    kind_id, StructuredFieldType as Field, StructuredFieldValue, StructuredInfoType as Type,
    StructuredInfoValue as Owned, StructuredVariantCase,
};
use alloc::{vec, vec::Vec};
#[test]
fn nested_nominal_record_and_variant_match_original_canonical_composition() {
    let count = Type::leaf(kind_id("value/u64")).unwrap();
    let empty = Type::leaf(kind_id("value/empty")).unwrap();
    let record = Type::record(
        kind_id("test/composed-record"),
        vec![
            Field::new("count", count.clone()).unwrap(),
            Field::new("unit", empty.clone()).unwrap(),
        ],
    )
    .unwrap();
    let nominal = Type::nominal(kind_id("test/nominal"), record.clone()).unwrap();
    let root = Type::variant(
        kind_id("test/result"),
        vec![StructuredVariantCase::new("recorded", nominal.clone()).unwrap()],
    )
    .unwrap();
    let count_bytes = crate::encode_count(u64::MAX);
    let owned = Owned::variant(
        root.clone(),
        "recorded",
        Owned::nominal(
            nominal,
            Owned::record(
                record,
                vec![
                    StructuredFieldValue::new(
                        "count",
                        Owned::leaf(count, count_bytes.to_vec()).unwrap(),
                    )
                    .unwrap(),
                    StructuredFieldValue::new("unit", Owned::leaf(empty, Vec::new()).unwrap())
                        .unwrap(),
                ],
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let fields = [
        BorrowedStructuredValue::Leaf(&count_bytes),
        BorrowedStructuredValue::Empty,
    ];
    let payload = BorrowedStructuredValue::Record(&fields);
    let value = BorrowedStructuredValue::Variant {
        tag: "recorded",
        payload: &payload,
    };
    let schema = root.canonical_bytes().unwrap();
    let writer = PreparedLinkedStructuredComposer::new(&schema, owned.len()).unwrap();
    let mut output = vec![0; owned.len()];
    assert_eq!(writer.encode_into(value, &mut output).unwrap(), owned);
    validate_canonical_structured_value(&output).unwrap();
}
#[test]
fn preflight_refusals_preserve_caller_buffer_and_valid_empty_is_not_an_arbitrary_leaf() {
    let schema = Type::leaf(kind_id("value/u64"))
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let writer = PreparedLinkedStructuredComposer::new(&schema, 64).unwrap();
    let mut output = [0xa5; 64];
    for value in [
        BorrowedStructuredValue::Empty,
        BorrowedStructuredValue::Leaf(&[0; 7]),
        BorrowedStructuredValue::Record(&[]),
    ] {
        assert!(writer.encode_into(value, &mut output).is_err());
        assert_eq!(output, [0xa5; 64]);
    }
    let count = crate::encode_count(1);
    let mut short = [0xa5; 1];
    assert!(writer
        .encode_into(BorrowedStructuredValue::Leaf(&count), &mut short)
        .is_err());
    assert_eq!(short, [0xa5; 1]);
    assert!(PreparedLinkedStructuredComposer::new(&[0; 3], 64).is_err());
}
#[test]
fn wrong_record_count_and_unknown_variant_are_preflight_refusals() {
    let empty = Type::leaf(kind_id("value/empty")).unwrap();
    let record = Type::record(
        kind_id("test/record"),
        vec![Field::new("unit", empty.clone()).unwrap()],
    )
    .unwrap();
    let root = Type::variant(
        kind_id("test/variant"),
        vec![StructuredVariantCase::new("known", record).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let writer = PreparedLinkedStructuredComposer::new(&root, 512).unwrap();
    let mut output = [0xa5; 512];
    let payload = BorrowedStructuredValue::Record(&[]);
    for tag in ["known", "unknown"] {
        assert!(writer
            .encode_into(
                BorrowedStructuredValue::Variant {
                    tag,
                    payload: &payload
                },
                &mut output
            )
            .is_err());
        assert_eq!(output, [0xa5; 512]);
    }
}

#[test]
fn fixed_collection_matches_owned_bytes_and_preflights_count_and_element() {
    let octet = Type::leaf(kind_id("value/u8")).unwrap();
    let ty = Type::collection(octet.clone(), Some(2)).unwrap();
    let owned = Owned::collection(
        ty.clone(),
        vec![
            Owned::leaf(octet.clone(), vec![0]).unwrap(),
            Owned::leaf(octet, vec![255]).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let schema = ty.canonical_bytes().unwrap();
    let writer = PreparedLinkedStructuredComposer::new(&schema, owned.len()).unwrap();
    let mut output = vec![0; owned.len()];
    let values = [
        BorrowedStructuredValue::Leaf(&[0]),
        BorrowedStructuredValue::Leaf(&[255]),
    ];
    assert_eq!(
        writer
            .encode_into(BorrowedStructuredValue::Collection(&values), &mut output)
            .unwrap(),
        owned
    );
    for values in [
        &values[..1],
        &[
            BorrowedStructuredValue::Leaf(&[0, 1]),
            BorrowedStructuredValue::Leaf(&[255]),
        ][..],
    ] {
        output.fill(0xa5);
        assert!(writer
            .encode_into(BorrowedStructuredValue::Collection(values), &mut output)
            .is_err());
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
}
