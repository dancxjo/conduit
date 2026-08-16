use conduit_core::{
    KindId, StructuredFieldType, StructuredInfoType, StructuredVariantCase,
    UnmatchedVariantDisposition,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, CheckedCordStage,
    ProfileCatalog, StartupCatalog,
};

fn note_type() -> StructuredInfoType {
    let count = StructuredInfoType::leaf(KindId::from("value/count@1")).unwrap();
    StructuredInfoType::record(
        KindId::from("music/note@1"),
        vec![
            StructuredFieldType::new("pitch", count.clone()).unwrap(),
            StructuredFieldType::new("velocity", count).unwrap(),
        ],
    )
    .unwrap()
}

fn selector_types() -> (StructuredInfoType, StructuredInfoType, StructuredInfoType) {
    let count = StructuredInfoType::leaf(KindId::from("value/count@1")).unwrap();
    let text = StructuredInfoType::leaf(KindId::from("value/text@1")).unwrap();
    let pitches = StructuredInfoType::collection(count.clone(), Some(3)).unwrap();
    let note = note_type();
    let rest = StructuredInfoType::leaf(KindId::from("music/rest@1")).unwrap();
    let event = StructuredInfoType::variant(
        KindId::from("music/event@1"),
        vec![
            StructuredVariantCase::new("note", note).unwrap(),
            StructuredVariantCase::new("rest", rest).unwrap(),
        ],
    )
    .unwrap();
    let feedback = StructuredInfoType::record(
        KindId::from("product/feedback@1"),
        vec![StructuredFieldType::new("status", text).unwrap()],
    )
    .unwrap();

    (pitches, event, feedback)
}

fn selector_catalog() -> StartupCatalog {
    let (pitches, event, feedback) = selector_types();
    let mut catalog = StartupCatalog::new();
    for (name, value_type) in [
        ("PitchTable", pitches),
        ("MusicEvent", event),
        ("Feedback", feedback),
    ] {
        catalog.insert_structured_type(name, value_type).unwrap();
    }
    catalog
}

fn check(source: &str) -> conduit_form::CheckedSyntaxDocument {
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    check_syntax_document(&parsed, &selector_catalog()).expect("selector Form checks")
}

#[test]
fn required_domain_selectors_are_statically_typed_cord_stages() {
    let checked = check(
        "form examples {\n button > index(PitchTable[1]) > synth\n events > select(MusicEvent.note, unmatched=drop) > notes\n feedback > project(Feedback.status) > presentation\n}\n",
    );
    let cords = &checked.forms[0].cords;
    assert_eq!(cords.len(), 3);

    let CheckedCordStage::StructuredSelector { selector, .. } = &cords[0].stages[1] else {
        panic!("pitch-table index must remain a typed Cord stage");
    };
    assert_eq!(selector.input_type(), &selector_types().0);
    assert_eq!(
        selector.output_type(),
        &StructuredInfoType::leaf(KindId::from("value/count@1")).unwrap()
    );

    let CheckedCordStage::StructuredSelector { selector, .. } = &cords[1].stages[1] else {
        panic!("music variant selection must remain a typed Cord stage");
    };
    assert_eq!(
        selector.unmatched_disposition(),
        Some(UnmatchedVariantDisposition::Drop)
    );
    assert_eq!(selector.output_type(), &note_type());

    let CheckedCordStage::StructuredSelector { selector, .. } = &cords[2].stages[1] else {
        panic!("feedback projection must remain a typed Cord stage");
    };
    assert_eq!(
        selector.output_type(),
        &StructuredInfoType::leaf(KindId::from("value/text@1")).unwrap()
    );
    assert!(checked.forms[0].gears.is_empty());
}

#[test]
fn selector_identity_ignores_trivia_but_includes_unmatched_policy() {
    let first =
        check("form choose {\n input > select(MusicEvent.note, unmatched=drop) > output\n}\n");
    let trivia = check(
        "# same meaning\nform choose {\n input > select( MusicEvent.note , unmatched = drop ) > output\n}\n",
    );
    let refusal =
        check("form choose {\n input > select(MusicEvent.note, unmatched=refuse) > output\n}\n");

    assert_ne!(first.source_document_id, trivia.source_document_id);
    assert_eq!(
        first.forms[0].checked_form_id,
        trivia.forms[0].checked_form_id
    );
    assert_ne!(
        first.forms[0].checked_form_id,
        refusal.forms[0].checked_form_id
    );
}

#[test]
fn invalid_members_bounds_and_policies_refuse_explicitly() {
    let cases = [
        ("index(PitchTable[3])", "outside the exact collection bound"),
        (
            "project(Feedback.missing)",
            "unknown structured selector field",
        ),
        (
            "select(MusicEvent.chord, unmatched=drop)",
            "unknown structured selector variant tag",
        ),
        ("index(Feedback[0])", "requires a collection type"),
        ("index(PitchTable[65536])", "finite u16 range"),
        ("select(MusicEvent.note, unmatched=skip)", "drop or refuse"),
    ];
    for (selector, message) in cases {
        let source = format!("form bad {{\n input > {selector} > output\n}}\n");
        let parsed = parse_syntax_document(&source);
        if let Some(error) = parsed.diagnostics.first() {
            assert!(error.message.contains(message), "{}", error.message);
            continue;
        }
        let error = check_syntax_document(&parsed, &selector_catalog()).unwrap_err();
        assert_eq!(error.code, "CND-FRM-052");
        assert!(error.message.contains(message), "{}", error.message);
        let owned = if selector.contains("65536") {
            "65536"
        } else {
            selector
        };
        assert_eq!(&source[error.span.start..error.span.end], owned);
    }
}

#[test]
fn pre_f3_expansion_refuses_instead_of_inventing_runtime_selector_gears() {
    let checked = check("form pending {\n project(Feedback.status) > output\n}\n");
    let error = expand_canonical_form(&checked, "pending", &ProfileCatalog::new()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-039");
    assert!(error.message.contains("structured selector"));
}
