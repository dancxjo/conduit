use super::*;
use conduit_core::PreparedBorrowedStructuredSelector as Prepared;
#[path = "../../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;

fn prepared(selector: &StructuredSelector) -> Prepared {
    let material = Box::leak(selector.canonical_bytes().unwrap().into_boxed_slice());
    Prepared::new(material, 4096, 4096).unwrap()
}
fn parity(selector: StructuredSelector, values: &[StructuredInfoValue]) {
    let linked = prepared(&selector);
    assert_eq!(
        linked.input_type_bytes(),
        selector.input_type().canonical_bytes().unwrap()
    );
    assert_eq!(
        linked.output_type_bytes(),
        selector.output_type().canonical_bytes().unwrap()
    );
    let mut output = Vec::with_capacity(4096);
    let mut separated = Vec::with_capacity(4096);
    for input in values {
        let bytes = input.canonical_bytes().unwrap();
        let expected = selector.select(input).unwrap();
        let (actual, cost) = probe::observe(|| linked.select_canonical_into(&bytes, &mut output));
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
        let schema = linked.input_type_bytes();
        let (parts, cost) = probe::observe(|| {
            linked.select_parts_into(schema, &bytes[schema.len()..], &mut separated)
        });
        assert_eq!((cost.allocations, cost.reallocations), (0, 0));
        assert_eq!(parts, actual);
        assert_eq!(separated, output);
        match expected {
            StructuredSelection::Matched(value) => {
                assert_eq!(actual, Ok(StructuredCanonicalSelection::Matched));
                assert_eq!(output, value.canonical_bytes().unwrap());
            }
            StructuredSelection::Unmatched(disposition) => {
                assert_eq!(
                    actual,
                    Ok(StructuredCanonicalSelection::Unmatched(disposition))
                );
            }
        }
    }
}
#[test]
fn all_four_linked_operations_match_owned_selection_without_allocating() {
    parity(
        StructuredSelector::field(note_type(), "pitch").unwrap(),
        &[note(62)],
    );
    parity(
        StructuredSelector::index(named_pitch_table_type(), 1).unwrap(),
        &[named_pitch_table()],
    );
    for unmatched in [
        UnmatchedVariantDisposition::Drop,
        UnmatchedVariantDisposition::Refuse,
    ] {
        parity(
            StructuredSelector::variant(music_event_type(), "note", unmatched).unwrap(),
            &[music_event("note"), music_event("control")],
        );
        for polarity in [false, true] {
            parity(
                StructuredSelector::record_field_values(
                    notice_type(),
                    "delivery",
                    vec![b"visible".to_vec()],
                    polarity,
                    unmatched,
                )
                .unwrap(),
                &[notice("visible"), notice("silent")],
            );
        }
    }
    let nominal_record = StructuredInfoType::nominal("type/Note@1".into(), note_type()).unwrap();
    parity(
        StructuredSelector::field(nominal_record.clone(), "pitch").unwrap(),
        &[StructuredInfoValue::nominal(nominal_record, note(62)).unwrap()],
    );
    let nominal_variant =
        StructuredInfoType::nominal("type/Event@1".into(), music_event_type()).unwrap();
    parity(
        StructuredSelector::variant(
            nominal_variant.clone(),
            "note",
            UnmatchedVariantDisposition::Drop,
        )
        .unwrap(),
        &[StructuredInfoValue::nominal(nominal_variant, music_event("note")).unwrap()],
    );
    let nominal = StructuredInfoType::nominal("type/Notice@1".into(), notice_type()).unwrap();
    parity(
        StructuredSelector::record_field_values(
            nominal.clone(),
            "delivery",
            vec![b"visible".to_vec()],
            true,
            UnmatchedVariantDisposition::Drop,
        )
        .unwrap(),
        &[StructuredInfoValue::nominal(nominal, notice("visible")).unwrap()],
    );
}
#[test]
fn full_validation_refuses_a_malformed_unselected_leaf_and_preserves_output() {
    let ty = StructuredInfoType::record(
        "fixture/two-octets".into(),
        vec![
            StructuredFieldType::new("a", leaf("value/u8")).unwrap(),
            StructuredFieldType::new("b", leaf("value/u8")).unwrap(),
        ],
    )
    .unwrap();
    let value = StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new(
                "a",
                StructuredInfoValue::leaf(leaf("value/u8"), vec![1]).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "b",
                StructuredInfoValue::leaf(leaf("value/u8"), vec![2]).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let linked = prepared(&StructuredSelector::field(ty, "a").unwrap());
    let mut malformed = value.canonical_bytes().unwrap();
    let length = malformed.len() - 5;
    malformed[length] = 2;
    malformed.push(0);
    let mut output = Vec::with_capacity(4096);
    output.extend_from_slice(b"previous");
    let (result, cost) = probe::observe(|| linked.select_canonical_into(&malformed, &mut output));
    assert_eq!(
        result,
        Err(StructuredSelectorRefusal::MalformedCheckedValue)
    );
    assert_eq!(output, b"previous");
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let (result, cost) = probe::observe(|| {
        linked.select_parts_into(
            linked.input_type_bytes(),
            &malformed[linked.input_type_bytes().len()..],
            &mut output,
        )
    });
    assert_eq!(
        result,
        Err(StructuredSelectorRefusal::MalformedCheckedValue)
    );
    assert_eq!(output, b"previous");
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let foreign = leaf("value/u8").canonical_bytes().unwrap();
    assert_eq!(
        linked.select_parts_into(
            &foreign,
            &malformed[linked.input_type_bytes().len()..],
            &mut output
        ),
        Err(StructuredSelectorRefusal::WrongInputType)
    );
    assert_eq!(output, b"previous");
}
#[test]
fn linked_limits_short_sequences_and_wrong_schemas_refuse_without_growing() {
    let ty = StructuredInfoType::sequence(leaf("value/count"), 3).unwrap();
    let linked = prepared(&StructuredSelector::index(ty.clone(), 2).unwrap());
    let short = StructuredInfoValue::sequence(ty, vec![count(1)])
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let mut output = Vec::with_capacity(4096);
    assert_eq!(
        linked.select_canonical_into(&short, &mut output),
        Err(StructuredSelectorRefusal::IndexOutOfRange)
    );
    assert_eq!(
        linked.select_canonical_into(&count(1).canonical_bytes().unwrap(), &mut output),
        Err(StructuredSelectorRefusal::WrongInputType)
    );
    let selector = StructuredSelector::index(pitch_table_type(), 0).unwrap();
    let material = Box::leak(selector.canonical_bytes().unwrap().into_boxed_slice());
    assert!(Prepared::new(material, 1, 4096).is_err());
    assert!(Prepared::new(material, 4096, 1).is_err());
    let linked = prepared(&selector);
    let bytes = pitch_table().canonical_bytes().unwrap();
    let mut small = Vec::new();
    assert_eq!(
        linked.select_canonical_into(&bytes, &mut small),
        Err(StructuredSelectorRefusal::CanonicalEncodingTooLarge)
    );
    assert_eq!(small.capacity(), 0);
    let mut noncanonical = selector.canonical_bytes().unwrap();
    noncanonical.push(0);
    assert!(Prepared::new(Box::leak(noncanonical.into_boxed_slice()), 4096, 4096).is_err());
}
