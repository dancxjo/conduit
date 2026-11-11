//! Forwarding proof only; Source fixtures separately prove class meaning.
use super::*;
use alloc::vec;
#[test]
fn source_payloads_and_continuations_are_forwarded_without_play_allocation() {
    let octet = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let keyboard = StructuredInfoType::record(
        kind_id("fixture/keyboard-work"),
        vec![
            StructuredFieldType::new("input", octet.clone()).unwrap(),
            StructuredFieldType::new("continuation", octet.clone()).unwrap(),
        ],
    )
    .unwrap();
    let work = StructuredInfoType::variant(
        kind_id("fixture/class-work"),
        vec![
            StructuredVariantCase::new("descriptor", octet.clone()).unwrap(),
            StructuredVariantCase::new("keyboard", keyboard.clone()).unwrap(),
        ],
    )
    .unwrap();
    let leaf = |n| StructuredInfoValue::leaf(octet.clone(), vec![n]).unwrap();
    let make = |input, continuation| {
        StructuredInfoValue::variant(
            work.clone(),
            "keyboard",
            StructuredInfoValue::record(
                keyboard.clone(),
                vec![
                    StructuredFieldValue::new("input", leaf(input)).unwrap(),
                    StructuredFieldValue::new("continuation", leaf(continuation)).unwrap(),
                ],
            )
            .unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    };
    let first = make(11, 21);
    let second = make(12, 22);
    let descriptor = StructuredInfoValue::variant(work, "descriptor", leaf(31))
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let expected_first = leaf(11).canonical_bytes().unwrap();
    let expected_continuations = [
        leaf(21).canonical_bytes().unwrap(),
        leaf(22).canonical_bytes().unwrap(),
    ];
    let expected_descriptor = leaf(31).canonical_bytes().unwrap();
    let continuation_kind = kind_id("fixture/continuation");
    let mut route = ClassWork::prepare(kind_id("fixture/descriptor"), continuation_kind.clone());
    let allocations = crate::test_allocations::allocations(|| {
        for (input, expected) in [
            (&first, &expected_continuations[0]),
            (&second, &expected_continuations[1]),
        ] {
            let Work::KeyboardRetained(value) = route.route(input).unwrap() else {
                panic!("wrong work");
            };
            assert_eq!(value.value_kind, continuation_kind);
            assert_eq!(&value.encoded, expected);
            assert_eq!(route.keyboard(), Some(expected_first.as_slice()));
        }
        let Work::Descriptor(value) = route.route(&descriptor).unwrap() else {
            panic!("wrong work");
        };
        assert_eq!(value.encoded, expected_descriptor);
    });
    assert_eq!(allocations, 0);
}
