extern crate std;
use super::*;
use crate::{kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoValue};

#[allow(dead_code)]
mod probe {
    use super::std;
    use std::thread_local;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plot/tests/common/allocation_probe.rs"
    ));
    pub fn live() -> usize {
        OBSERVATION.with(|state| state.get().live_bytes)
    }
}
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;

fn leaf(name: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(name)).unwrap()
}
fn record_type(name: &str) -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id(name),
        vec![StructuredFieldType::new("value", leaf("value/u64")).unwrap()],
    )
    .unwrap()
}
fn record(name: &str, number: u64) -> Vec<u8> {
    StructuredInfoValue::record(
        record_type(name),
        vec![StructuredFieldValue::new(
            "value",
            StructuredInfoValue::leaf(leaf("value/u64"), number.to_le_bytes().to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn structured_encoder() -> PreparedTypedTuplePairEncoder {
    PreparedTypedTuplePairEncoder::new(
        record_type("test/pair-left"),
        1024,
        record_type("test/pair-right"),
        1024,
    )
    .unwrap()
}

#[test]
fn runtime_pair_keeps_exact_outputs_type_refusals_and_envelopes() {
    let mut original = structured_encoder();
    let mut execution = structured_encoder().into_runtime();
    assert_eq!(original.maximum_bytes(), execution.maximum_bytes());
    let left = record("test/pair-left", 11);
    let right = record("test/pair-right", 22);
    let wrong = record("test/other-pair-member", 22);
    let oversized = vec![0; 1025];
    let mut malformed = right.clone();
    malformed[0] ^= 0xff;
    let ((), cost) = probe::observe(|| {
        for (l, r) in [
            (&left[..], &right[..]),
            (&left, &wrong),
            (&left, &oversized),
            (&left, &malformed),
            (&oversized, &right),
        ] {
            assert_eq!(original.encode(l, r), execution.encode(l, r));
        }
        let expected = original.encode(&left, &right).unwrap();
        assert_eq!(execution.encode(&left, &right).unwrap(), expected);
        assert_eq!(execution.encoded(), original.encoded());
    });
    assert_eq!(cost.allocation_attempts, 0);
    assert_eq!(cost.reallocations, 0);
}

#[test]
fn consuming_boundary_preserves_already_encoded_value_and_does_not_allocate() {
    let mut original = structured_encoder();
    original
        .encode(&record("test/pair-left", 1), &record("test/pair-right", 2))
        .unwrap();
    let expected = original.encoded().to_vec();
    let (execution, cost) = probe::observe(|| original.into_runtime());
    assert_eq!(cost.allocation_attempts, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(execution.encoded(), expected);
}

#[test]
fn primitive_validation_and_execution_remain_allocation_free() {
    let mut original =
        PreparedTypedTuplePairEncoder::new(leaf("value/u8"), 1, leaf("value/bool"), 1).unwrap();
    let mut execution =
        PreparedTypedTuplePairEncoder::new(leaf("value/u8"), 1, leaf("value/bool"), 1)
            .unwrap()
            .into_runtime();
    let ((), cost) = probe::observe(|| {
        for octet in 0..=255 {
            for boolean in [0, 1, 2] {
                assert_eq!(
                    original.encode(&[octet], &[boolean]),
                    execution.encode(&[octet], &[boolean])
                );
            }
        }
        assert_eq!(
            original.encode(&[0, 1], &[1]),
            execution.encode(&[0, 1], &[1])
        );
    });
    assert_eq!(cost.allocation_attempts, 0);
    assert_eq!(cost.reallocations, 0);
}

#[test]
fn exclusive_inspection_tree_is_released_at_the_runtime_boundary() {
    let ((execution, before, after), cost) = probe::observe(|| {
        let original = structured_encoder();
        let before = probe::live();
        let execution = original.into_runtime();
        let after = probe::live();
        (execution, before, after)
    });
    assert!(
        after < before,
        "the rich inspection tree must not remain reachable"
    );
    assert!(cost.deallocations > 0);
    assert!(execution.maximum_bytes() > 2048);
}

#[test]
fn fixed_schema_composition_keeps_selected_envelope_without_reserving_it_all() {
    let mut preparation = structured_encoder();
    let selected_maximum = preparation.maximum_bytes();
    assert!(preparation.composer.prepared_heap_bytes() < selected_maximum as usize);
    let left = record("test/pair-left", u64::MAX);
    let right = record("test/pair-right", u64::MAX);
    let expected = preparation.encode(&left, &right).unwrap().to_vec();
    let mut runtime = preparation.into_runtime();
    assert_eq!(runtime.maximum_bytes(), selected_maximum);
    let ((), cost) = probe::observe(|| {
        assert_eq!(runtime.encode(&left, &right).unwrap(), expected);
    });
    assert_eq!(cost.allocation_attempts, 0);
    assert_eq!(cost.reallocations, 0);
}

#[test]
fn fixed_pair_output_bound_matches_actual_canonical_value_without_changing_input_limits() {
    let mut pair = structured_encoder();
    let selected = pair.maximum_bytes();
    let exact = pair.maximum_encoded_bytes();
    assert!(exact < selected);
    for number in [0, 17, u64::MAX] {
        assert_eq!(
            pair.encode(
                &record("test/pair-left", number),
                &record("test/pair-right", number)
            )
            .unwrap()
            .len(),
            exact as usize
        );
    }
    assert_eq!(pair.maximum_bytes(), selected);
    assert!(pair
        .encode(&vec![0; 1025], &record("test/pair-right", 0))
        .is_err());
}

#[test]
fn unknown_leaf_pair_keeps_the_full_selected_output_envelope() {
    let pair = PreparedTypedTuplePairEncoder::new(
        leaf("test/variable-payload"),
        1024,
        leaf("value/u64"),
        8,
    )
    .unwrap();
    assert_eq!(pair.maximum_encoded_bytes(), pair.maximum_bytes());
}

#[test]
fn schema_separated_composition_and_validation_do_not_allocate_during_execution() {
    use crate::{
        BorrowedStructuredValue, BorrowedStructuredValueValidator, PreparedLinkedStructuredComposer,
    };
    let ty = record_type("test/shared-schema");
    let schema = ty.canonical_bytes().unwrap();
    let original = record("test/shared-schema", u64::MAX);
    let checked = crate::validate_canonical_structured_value(&original).unwrap();
    let composer = PreparedLinkedStructuredComposer::new(&schema, original.len()).unwrap();
    let validator = BorrowedStructuredValueValidator::new(&schema, original.len()).unwrap();
    let mut output = vec![0; checked.value_node().len()];
    let number = u64::MAX.to_le_bytes();
    let fields = [BorrowedStructuredValue::Leaf(&number)];
    let ((), cost) = probe::observe(|| {
        for _ in 0..32 {
            let node = composer
                .encode_node_into(BorrowedStructuredValue::Record(&fields), &mut output)
                .unwrap();
            assert_eq!(node, checked.value_node());
            assert_eq!(validator.validate_node(node).unwrap(), checked);
        }
    });
    assert_eq!(cost.allocation_attempts, 0);
    assert_eq!(cost.reallocations, 0);
}
