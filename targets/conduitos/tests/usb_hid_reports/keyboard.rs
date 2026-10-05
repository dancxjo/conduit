use super::common::*;
use conduit_core::*;
use conduit_plot::PreparedPortableExpressionEvaluator;

#[test]
fn keyboard_length_error_usages_and_duplicates_remain_distinct() {
    let p = program("usb-hid-keyboard-frame");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let mut cases = vec![
        (vec![], 0, false, "short"),
        (vec![0; 7], 7, false, "short"),
        (vec![0; 8], 7, false, "malformed"),
        (vec![0; 8], u64::MAX, false, "malformed"),
        (vec![3, 0, 4, 4, 0, 0, 0, 0], 8, false, "duplicate"),
        (vec![255, 127, 0, 0, 0, 0, 0, 0], 8, false, "keyboard"),
        (vec![255, 127, 0, 0, 0, 0, 0, 0], 8, true, "reserved"),
    ];
    for position in 2..8 {
        for error in 1..=3 {
            let mut wire = vec![0; 8];
            wire[position] = error;
            cases.push((wire, 8, false, "report-error"));
        }
    }
    for left in 2..8 {
        for right in left + 1..8 {
            let mut wire = vec![0; 8];
            wire[left] = 255;
            wire[right] = 255;
            cases.push((wire, 8, false, "duplicate"));
        }
    }
    for (wire, actual, strict, expected) in cases {
        let input = request(&p, &wire, actual, strict);
        let ordinary = p.evaluate(&input).unwrap();
        let result = prepared.evaluate(&input).unwrap();
        assert_eq!(result, ordinary);
        tag(result, expected);
        if expected == "report-error" {
            let value = validate_canonical_structured_value(result)
                .unwrap()
                .variant_payload("report-error")
                .unwrap()
                .unwrap();
            assert_eq!(
                value.primitive_bytes("value/u8").unwrap(),
                [wire
                    .iter()
                    .copied()
                    .find(|byte| (1..=3).contains(byte))
                    .unwrap()]
            );
        }
    }
}

fn report(schema: &StructuredInfoType, keys: [u8; 6]) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
        panic!("report record");
    };
    let octet = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let keys_type = fields
        .iter()
        .find(|field| field.name() == "keys")
        .unwrap()
        .value_type();
    StructuredInfoValue::record(
        schema.clone(),
        vec![
            StructuredFieldValue::new(
                "modifiers",
                StructuredInfoValue::leaf(octet.clone(), vec![0xff]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "keys",
                StructuredInfoValue::collection(
                    keys_type.clone(),
                    keys.into_iter()
                        .map(|key| StructuredInfoValue::leaf(octet.clone(), vec![key]).unwrap())
                        .collect(),
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

fn permutations(keys: &mut [u8; 6], at: usize, out: &mut Vec<[u8; 6]>) {
    if at == keys.len() {
        out.push(*keys);
        return;
    }
    for index in at..keys.len() {
        keys.swap(at, index);
        permutations(keys, at + 1, out);
        keys.swap(at, index);
    }
}

#[test]
fn complete_source_sort_orders_every_permutation_without_allocations() {
    let even = program("usb-hid-keyboard-sort-even");
    let odd = program("usb-hid-keyboard-sort-odd");
    let mut even_prepared = PreparedPortableExpressionEvaluator::new(&even).unwrap();
    let mut odd_prepared = PreparedPortableExpressionEvaluator::new(&odd).unwrap();
    let mut cases = vec![];
    permutations(&mut [4, 5, 6, 7, 8, 9], 0, &mut cases);
    assert_eq!(cases.len(), 720);
    cases.extend([[255, 254, 6, 4, 0, 0], [0; 6], [255, 0, 255, 0, 1, 0]]);
    let inputs: Vec<_> = cases
        .iter()
        .map(|keys| report(&even.input_type, *keys))
        .collect();
    let mut input = Vec::with_capacity(4096);
    let mut output = Vec::with_capacity(4096);
    let allocations = crate::allocation::allocations(|| {
        for (keys, bytes) in cases.iter().zip(&inputs) {
            input.clear();
            input.extend_from_slice(bytes);
            for _ in 0..3 {
                output.clear();
                output.extend_from_slice(even_prepared.evaluate(&input).unwrap());
                std::mem::swap(&mut input, &mut output);
                output.clear();
                output.extend_from_slice(odd_prepared.evaluate(&input).unwrap());
                std::mem::swap(&mut input, &mut output);
            }
            let value = validate_canonical_structured_value(&input).unwrap();
            assert_eq!(
                value
                    .record_field("modifiers")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u8")
                    .unwrap(),
                [255]
            );
            let actual = value.record_field("keys").unwrap().unwrap();
            let mut expected = *keys;
            expected.sort_unstable();
            for (at, key) in expected.iter().enumerate() {
                assert_eq!(
                    actual
                        .collection_index(u16::try_from(at).unwrap())
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u8")
                        .unwrap(),
                    [*key]
                );
            }
        }
    });
    assert_eq!(allocations, 0);
}
