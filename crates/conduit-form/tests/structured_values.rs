use conduit_core::{KindId, StructuredFieldType, StructuredInfoType, StructuredVariantCase};
use conduit_form::{
    check_syntax_document, parse_syntax_document, CanonicalStartupValue, KindSignature,
    StartupCatalog, StartupParameterSignature,
};

fn structured_catalog() -> StartupCatalog {
    let count = StructuredInfoType::leaf(KindId::from("value/count@1")).unwrap();
    let pitches = StructuredInfoType::collection(count.clone(), Some(3)).unwrap();
    let note = StructuredInfoType::record(
        KindId::from("music/note-on@1"),
        vec![
            StructuredFieldType::new("pitches", pitches).unwrap(),
            StructuredFieldType::new("velocity", count).unwrap(),
        ],
    )
    .unwrap();
    let event = StructuredInfoType::variant(
        KindId::from("music/event@1"),
        vec![StructuredVariantCase::new("note_on", note).unwrap()],
    )
    .unwrap();

    let mut catalog = StartupCatalog::new();
    catalog.insert_structured_type("MusicEvent", event).unwrap();
    catalog
        .insert(KindSignature {
            kind: "test/consume-event".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "event".into(),
                value_type: "MusicEvent".into(),
                default: None,
            }],
        })
        .unwrap();
    catalog
}

fn defaulted_structured_catalog() -> StartupCatalog {
    let mut catalog = structured_catalog();
    catalog
        .insert(KindSignature {
            kind: "test/default-event".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "event".into(),
                value_type: "MusicEvent".into(),
                default: Some("note_on({ pitches: [60, 62, 64], velocity: 96 })".into()),
            }],
        })
        .unwrap();
    catalog
}

fn check(source: &str) -> conduit_form::CheckedSyntaxDocument {
    let parsed = parse_syntax_document(source);
    check_syntax_document(&parsed, &structured_catalog()).expect("structured Form checks")
}

#[test]
fn collection_record_and_variant_literals_become_one_concrete_f0_value() {
    let checked = check(
        "form instrument {\n sink: test/consume-event(note_on({ velocity: 96, pitches: [60, 62, 64] }))\n}\n",
    );
    let CanonicalStartupValue::Structured(value) =
        &checked.forms[0].gears[0].startup_bindings[0].value
    else {
        panic!("structured syntax must not remain opaque source text");
    };

    assert!(value.try_concrete().is_some());
}

#[test]
fn trivia_and_record_field_order_do_not_change_checked_identity() {
    let first = check(
        "form instrument {\n sink: test/consume-event(note_on({ velocity: 96, pitches: [60, 62, 64] }))\n}\n",
    );
    let second = check(
        "form instrument {\n sink: test/consume-event( note_on( { pitches: [ 60,62,64 ], velocity: 96 } ) )\n}\n",
    );

    assert_ne!(first.source_document_id, second.source_document_id);
    assert_eq!(
        first.forms[0].checked_form_id,
        second.forms[0].checked_form_id
    );
    assert_eq!(first.forms[0].gears, second.forms[0].gears);
}

#[test]
fn structured_local_and_nested_form_parameter_resolve_without_runtime_evaluation() {
    let checked = check(
        "form instrument (\n velocity: Count\n) {\n event = note_on({ pitches: [60, 62, 64], velocity: velocity })\n sink: test/consume-event(event)\n}\n",
    );
    let CanonicalStartupValue::Structured(value) =
        &checked.forms[0].gears[0].startup_bindings[0].value
    else {
        panic!("local is checked under its consuming exact type");
    };

    assert!(value.try_concrete().is_none());
    assert!(value.canonical_identity().contains("velocity"));
}

#[test]
fn structured_catalog_and_form_defaults_are_checked_semantic_values() {
    let parsed = parse_syntax_document("form defaulted {\n sink: test/default-event\n}\n");
    let checked = check_syntax_document(&parsed, &defaulted_structured_catalog()).unwrap();
    let CanonicalStartupValue::Structured(default) =
        &checked.forms[0].gears[0].startup_bindings[0].value
    else {
        panic!("catalog default must be checked rather than retained as text");
    };
    assert!(default.try_concrete().is_some());

    let source = "form reusable (\n velocity: Count = 96\n event: MusicEvent = note_on({ velocity: velocity, pitches: [60, 62, 64] })\n) {\n}\n";
    let parsed = parse_syntax_document(source);
    let checked = check_syntax_document(&parsed, &structured_catalog()).unwrap();
    let Some(CanonicalStartupValue::Structured(default)) =
        &checked.forms[0].startup_parameters[1].default
    else {
        panic!("Form-face default must use the same checked substrate");
    };
    assert!(default.try_concrete().is_some());

    let source = "form bad (\n event: MusicEvent = note_off({ pitches: [60, 62, 64], velocity: 96 })\n) {\n}\n";
    let parsed = parse_syntax_document(source);
    let error = check_syntax_document(&parsed, &structured_catalog()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-051");
    assert_eq!(
        &source[error.span.start..error.span.end],
        "note_off({ pitches: [60, 62, 64], velocity: 96 })"
    );
}

#[test]
fn duplicate_unknown_missing_and_unknown_tag_have_precise_diagnostics() {
    let cases = [
        (
            "note_on({ pitches: [60, 62, 64], velocity: 96, velocity: 97 })",
            "duplicate structured field 'velocity'",
            "velocity",
        ),
        (
            "note_on({ pitches: [60, 62, 64], pressure: 96 })",
            "unknown structured field 'pressure'",
            "pressure",
        ),
        (
            "note_on({ pitches: [60, 62, 64] })",
            "missing field 'velocity'",
            "{ pitches: [60, 62, 64] }",
        ),
        (
            "note_off({ pitches: [60, 62, 64], velocity: 96 })",
            "unknown structured variant tag 'note_off'",
            "note_off",
        ),
    ];

    for (expression, message, owned_span) in cases {
        let source = format!("form bad {{\n sink: test/consume-event({expression})\n}}\n");
        let parsed = parse_syntax_document(&source);
        let error = check_syntax_document(&parsed, &structured_catalog()).unwrap_err();
        assert_eq!(error.code, "CND-FRM-051", "{expression}");
        assert!(error.message.contains(message), "{}", error.message);
        assert_eq!(&source[error.span.start..error.span.end], owned_span);
    }
}

#[test]
fn collection_length_leaf_type_and_runtime_port_fail_distinctly() {
    let cases = [
        (
            "note_on({ pitches: [60, 62], velocity: 96 })",
            "exact type requires 3",
        ),
        (
            "note_on({ pitches: [60, \"wrong\", 64], velocity: 96 })",
            "incompatible with exact leaf kind 'value/count@1'",
        ),
        (
            "note_on({ pitches: [60, 62, 64], velocity: 18446744073709551616 })",
            "incompatible with exact leaf kind 'value/count@1'",
        ),
    ];
    for (expression, message) in cases {
        let source = format!("form bad {{\n sink: test/consume-event({expression})\n}}\n");
        let parsed = parse_syntax_document(&source);
        let error = check_syntax_document(&parsed, &structured_catalog()).unwrap_err();
        assert_eq!(error.code, "CND-FRM-051");
        assert!(error.message.contains(message), "{}", error.message);
    }

    let source = "form bad (\n current: $Count > out: Count\n) {\n sink: test/consume-event(note_on({ pitches: [60, 62, 64], velocity: current }))\n}\n";
    let parsed = parse_syntax_document(source);
    let error = check_syntax_document(&parsed, &structured_catalog()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-051");
    assert!(error.message.contains("runtime port 'current'"));
    assert_eq!(&source[error.span.start..error.span.end], "current");
}

#[test]
fn parser_refuses_excessive_nesting_at_the_exact_nested_value() {
    let nested = "[".repeat(conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH)
        + "1"
        + &"]".repeat(conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH);
    let source = format!("form bad {{\n value = {nested}\n}}\n");
    let parsed = parse_syntax_document(&source);
    let error = parsed
        .diagnostics
        .first()
        .expect("over-depth source refuses");

    assert_eq!(error.code, "CND-FRM-019");
    assert!(error.message.contains("nesting exceeds the finite limit"));
    assert!(!&source[error.span.start..error.span.end].is_empty());
    assert_eq!(parsed.round_trip(), source);
}

#[test]
fn malformed_structured_literals_keep_lossless_source_and_deterministic_diagnostics() {
    for expression in [
        "[1, 2,]",
        "note_on()",
        "note_on({ pitches: [60, 62, 64], velocity: 96, })",
        "note_on({})",
    ] {
        let source = format!("form bad {{\n value = {expression}\n}}\n");
        let parsed = parse_syntax_document(&source);
        let error = parsed
            .diagnostics
            .first()
            .expect("malformed literal refuses");
        assert_eq!(error.code, "CND-FRM-019", "{expression}");
        assert!(!error.message.is_empty());
        assert_eq!(parsed.round_trip(), source);
        assert!(!parsed.tokens.is_empty());
    }
}

#[test]
fn leaf_and_total_canonical_byte_bounds_fail_before_checked_identity() {
    let blob = StructuredInfoType::leaf(KindId::from("test/blob@1")).unwrap();
    let batch = StructuredInfoType::collection(blob.clone(), Some(20)).unwrap();
    let mut catalog = structured_catalog();
    catalog.insert_structured_type("Blob", blob).unwrap();
    catalog.insert_structured_type("BlobBatch", batch).unwrap();
    for (kind, value_type) in [
        ("test/consume-blob", "Blob"),
        ("test/consume-batch", "BlobBatch"),
    ] {
        catalog
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![StartupParameterSignature {
                    name: "value".into(),
                    value_type: value_type.into(),
                    default: None,
                }],
            })
            .unwrap();
    }

    let oversized_leaf = "a".repeat(conduit_core::MAXIMUM_STRUCTURED_LEAF_BYTES + 1);
    let source = format!("form bad {{\n sink: test/consume-blob({oversized_leaf})\n}}\n");
    let parsed = parse_syntax_document(&source);
    let error = check_syntax_document(&parsed, &catalog).unwrap_err();
    assert_eq!(error.code, "CND-FRM-051");
    assert!(error.message.contains("leaf literal exceeds"));

    let item = "b".repeat(4_000);
    let items = core::iter::repeat_n(item, 20).collect::<Vec<_>>().join(",");
    let source = format!("form bad {{\n sink: test/consume-batch([{items}])\n}}\n");
    let parsed = parse_syntax_document(&source);
    let error = check_syntax_document(&parsed, &catalog).unwrap_err();
    assert_eq!(error.code, "CND-FRM-051");
    assert!(error.message.contains("canonical encoding bound"));

    let item = "c".repeat(4_000);
    let mut items = core::iter::repeat_n(item, 19).collect::<Vec<_>>();
    items.push("parameter".into());
    let source = format!(
        "form bad (\n parameter: Blob\n) {{\n sink: test/consume-batch([{}])\n}}\n",
        items.join(",")
    );
    let parsed = parse_syntax_document(&source);
    let error = check_syntax_document(&parsed, &catalog).unwrap_err();
    assert_eq!(error.code, "CND-FRM-051");
    assert!(error.message.contains("canonical encoding bound"));
}
