use conduit_core::{
    KindId, StructuredCanonicalSelection, StructuredFieldType, StructuredFieldValue,
    StructuredFlowSelection, StructuredFlowSelector, StructuredInfoType, StructuredInfoValue,
    StructuredSelection, StructuredSelector, StructuredSelectorRefusal, StructuredVariantCase,
    UnmatchedVariantDisposition,
};

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(KindId::from(kind)).unwrap()
}

fn count(value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(leaf("value/count"), value.to_le_bytes().to_vec()).unwrap()
}

fn text(value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(leaf("value/text"), value.as_bytes().to_vec()).unwrap()
}

fn pitch_table_type() -> StructuredInfoType {
    StructuredInfoType::collection(leaf("value/count"), Some(3)).unwrap()
}

fn pitch_table() -> StructuredInfoValue {
    StructuredInfoValue::collection(pitch_table_type(), vec![count(60), count(62), count(64)])
        .unwrap()
}

fn note_type() -> StructuredInfoType {
    StructuredInfoType::record(
        KindId::from("music/note@1"),
        vec![
            StructuredFieldType::new("pitch", leaf("value/count")).unwrap(),
            StructuredFieldType::new("velocity", leaf("value/count")).unwrap(),
        ],
    )
    .unwrap()
}

fn note(pitch: u64) -> StructuredInfoValue {
    StructuredInfoValue::record(
        note_type(),
        vec![
            StructuredFieldValue::new("pitch", count(pitch)).unwrap(),
            StructuredFieldValue::new("velocity", count(96)).unwrap(),
        ],
    )
    .unwrap()
}

fn music_event_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        KindId::from("music/event@1"),
        vec![
            StructuredVariantCase::new("control", leaf("value/count")).unwrap(),
            StructuredVariantCase::new("note", note_type()).unwrap(),
        ],
    )
    .unwrap()
}

fn music_event(tag: &str) -> StructuredInfoValue {
    let payload = if tag == "note" { note(64) } else { count(7) };
    StructuredInfoValue::variant(music_event_type(), tag, payload).unwrap()
}

fn notice_type() -> StructuredInfoType {
    StructuredInfoType::record(
        KindId::from("notice/thoughtful@1"),
        vec![
            StructuredFieldType::new("delivery", leaf("value/text")).unwrap(),
            StructuredFieldType::new("body", leaf("value/text")).unwrap(),
        ],
    )
    .unwrap()
}

fn notice(delivery: &str) -> StructuredInfoValue {
    StructuredInfoValue::record(
        notice_type(),
        vec![
            StructuredFieldValue::new("delivery", text(delivery)).unwrap(),
            StructuredFieldValue::new("body", text("Mind the step")).unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn button_index_selects_one_finite_pitch_without_numeric_coercion() {
    let selector = StructuredSelector::index(pitch_table_type(), 1).unwrap();
    let StructuredSelection::Matched(selected) = selector.select(&pitch_table()).unwrap() else {
        panic!("a valid fixed index must match");
    };

    assert_eq!(selected, count(62));
    assert_eq!(selector.output_type(), &leaf("value/count"));
    assert_eq!(selector.input_type(), &pitch_table_type());
}

#[test]
fn musical_variant_selection_has_exact_output_and_explicit_unmatched_paths() {
    let drop_selector = StructuredSelector::variant(
        music_event_type(),
        "note",
        UnmatchedVariantDisposition::Drop,
    )
    .unwrap();
    assert_eq!(drop_selector.output_type(), &note_type());
    assert!(matches!(
        drop_selector.select(&music_event("note")).unwrap(),
        StructuredSelection::Matched(value) if value == note(64)
    ));
    assert_eq!(
        drop_selector.select(&music_event("control")).unwrap(),
        StructuredSelection::Unmatched(UnmatchedVariantDisposition::Drop)
    );

    let refuse_selector = StructuredSelector::variant(
        music_event_type(),
        "note",
        UnmatchedVariantDisposition::Refuse,
    )
    .unwrap();
    let mut flow = StructuredFlowSelector::new(refuse_selector);
    assert_eq!(
        flow.offer(&music_event("control"), true),
        Err(StructuredSelectorRefusal::UnmatchedVariant)
    );
}

#[test]
fn feedback_field_projection_is_typed_for_presentation() {
    let feedback_type = StructuredInfoType::record(
        KindId::from("device/feedback@1"),
        vec![
            StructuredFieldType::new("status", leaf("value/text")).unwrap(),
            StructuredFieldType::new("temperature", leaf("value/count")).unwrap(),
        ],
    )
    .unwrap();
    let feedback = StructuredInfoValue::record(
        feedback_type.clone(),
        vec![
            StructuredFieldValue::new("temperature", count(42)).unwrap(),
            StructuredFieldValue::new("status", text("ready")).unwrap(),
        ],
    )
    .unwrap();
    let selector = StructuredSelector::field(feedback_type, "status").unwrap();

    assert_eq!(selector.output_type(), &leaf("value/text"));
    assert_eq!(
        selector.select(&feedback).unwrap(),
        StructuredSelection::Matched(text("ready"))
    );
}

#[test]
fn record_field_predicates_carry_the_whole_record_and_have_an_exact_inverse() {
    let visible = StructuredSelector::record_field_values(
        notice_type(),
        "delivery",
        vec![b"visible".to_vec()],
        true,
        UnmatchedVariantDisposition::Drop,
    )
    .unwrap();
    let otherwise = StructuredSelector::record_field_values(
        notice_type(),
        "delivery",
        vec![b"spoken".to_vec(), b"visible".to_vec()],
        false,
        UnmatchedVariantDisposition::Drop,
    )
    .unwrap();

    assert_eq!(
        visible.select(&notice("visible")).unwrap(),
        StructuredSelection::Matched(notice("visible"))
    );
    assert_eq!(
        visible.select(&notice("silent")).unwrap(),
        StructuredSelection::Unmatched(UnmatchedVariantDisposition::Drop)
    );
    assert_eq!(
        otherwise.select(&notice("visible")).unwrap(),
        StructuredSelection::Unmatched(UnmatchedVariantDisposition::Drop)
    );
    assert_eq!(
        otherwise.select(&notice("silent")).unwrap(),
        StructuredSelection::Matched(notice("silent"))
    );
    assert_eq!(otherwise.output_type(), &notice_type());
}

#[test]
fn record_field_predicate_round_trips_and_selects_without_play_time_allocation() {
    let selector = StructuredSelector::record_field_values(
        notice_type(),
        "delivery",
        vec![b"visible".to_vec()],
        true,
        UnmatchedVariantDisposition::Drop,
    )
    .unwrap();
    let encoded = selector.canonical_bytes().unwrap();
    assert_eq!(
        StructuredSelector::from_canonical_bytes(&encoded),
        Ok(selector.clone())
    );

    let input_type = selector.input_type().canonical_bytes().unwrap();
    let output_type = selector.output_type().canonical_bytes().unwrap();
    let expected = notice("visible").canonical_bytes().unwrap();
    let mut output = Vec::with_capacity(expected.len());
    assert_eq!(
        selector.select_canonical_into(&expected, &input_type, &output_type, &mut output),
        Ok(StructuredCanonicalSelection::Matched)
    );
    assert_eq!(output, expected);
    assert_eq!(
        selector.select_canonical_into(
            &notice("silent").canonical_bytes().unwrap(),
            &input_type,
            &output_type,
            &mut output,
        ),
        Ok(StructuredCanonicalSelection::Unmatched(
            UnmatchedVariantDisposition::Drop
        ))
    );
}

#[test]
fn out_of_range_unknown_and_wrong_input_paths_refuse_deterministically() {
    assert_eq!(
        StructuredSelector::index(pitch_table_type(), 3),
        Err(StructuredSelectorRefusal::IndexOutOfRange)
    );
    assert_eq!(
        StructuredSelector::field(note_type(), "duration"),
        Err(StructuredSelectorRefusal::UnknownField)
    );
    assert_eq!(
        StructuredSelector::variant(
            music_event_type(),
            "aftertouch",
            UnmatchedVariantDisposition::Drop,
        ),
        Err(StructuredSelectorRefusal::UnknownVariantTag)
    );
    let selector = StructuredSelector::index(pitch_table_type(), 0).unwrap();
    assert_eq!(
        selector.select(&count(0)),
        Err(StructuredSelectorRefusal::WrongInputType)
    );
}

#[test]
fn flow_selection_preserves_pressure_order_and_closure_without_retention() {
    let selector = StructuredSelector::variant(
        music_event_type(),
        "note",
        UnmatchedVariantDisposition::Drop,
    )
    .unwrap();
    let mut flow = StructuredFlowSelector::new(selector);
    let event = music_event("note");

    assert_eq!(
        flow.offer(&event, false).unwrap(),
        StructuredFlowSelection::Pressure
    );
    assert_eq!(
        flow.offer(&event, true).unwrap(),
        StructuredFlowSelection::Emitted(note(64))
    );
    assert_eq!(
        flow.offer(&music_event("control"), false).unwrap(),
        StructuredFlowSelection::UnmatchedDropped
    );
    assert_eq!(flow.close().unwrap(), StructuredFlowSelection::Closed);
    assert_eq!(
        flow.offer(&event, true),
        Err(StructuredSelectorRefusal::FlowAlreadyClosed)
    );
    assert_eq!(
        flow.close(),
        Err(StructuredSelectorRefusal::FlowAlreadyClosed)
    );
}

#[test]
fn selector_identity_is_canonical_and_includes_unmatched_policy() {
    let first = StructuredSelector::field(note_type(), "pitch").unwrap();
    let second = StructuredSelector::field(note_type(), "pitch").unwrap();
    assert_eq!(
        first.semantic_digest().unwrap(),
        second.semantic_digest().unwrap()
    );

    let drop = StructuredSelector::variant(
        music_event_type(),
        "note",
        UnmatchedVariantDisposition::Drop,
    )
    .unwrap();
    let refuse = StructuredSelector::variant(
        music_event_type(),
        "note",
        UnmatchedVariantDisposition::Refuse,
    )
    .unwrap();
    assert_ne!(
        drop.semantic_digest().unwrap(),
        refuse.semantic_digest().unwrap()
    );
}

#[test]
fn selector_configuration_round_trips_and_rejects_noncanonical_bytes() {
    let selector = StructuredSelector::field(note_type(), "velocity").unwrap();
    let encoded = selector.canonical_bytes().unwrap();
    assert_eq!(
        StructuredSelector::from_canonical_bytes(&encoded).unwrap(),
        selector
    );
    assert_eq!(
        StructuredSelector::from_canonical_hex(&selector.canonical_hex().unwrap()).unwrap(),
        selector
    );

    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        StructuredSelector::from_canonical_bytes(&trailing),
        Err(StructuredSelectorRefusal::MalformedCanonicalEncoding)
    );
}

#[test]
fn canonical_selection_validates_the_whole_input_and_writes_the_exact_value() {
    let selector = StructuredSelector::field(note_type(), "velocity").unwrap();
    let input_type = selector.input_type().canonical_bytes().unwrap();
    let output_type = selector.output_type().canonical_bytes().unwrap();
    let mut output = Vec::with_capacity(512);
    assert_eq!(
        selector.select_canonical_into(
            &note(64).canonical_bytes().unwrap(),
            &input_type,
            &output_type,
            &mut output,
        ),
        Ok(StructuredCanonicalSelection::Matched)
    );
    assert_eq!(output, count(96).canonical_bytes().unwrap());

    let mut malformed = note(64).canonical_bytes().unwrap();
    malformed.push(0);
    assert_eq!(
        selector.select_canonical_into(&malformed, &input_type, &output_type, &mut output),
        Err(StructuredSelectorRefusal::MalformedCheckedValue)
    );
}
