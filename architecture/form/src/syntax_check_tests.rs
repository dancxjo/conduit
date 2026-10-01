use crate::{
    check_syntax_document, parse_syntax_document, CanonicalStartupValue, KindSignature,
    StartupCatalog, StartupParameterSignature,
};
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, CheckedFront, PortDescriptor, PortDirection, PortTemporal, ValueConstraint,
    MAX_PATTERN_MATCH_STEPS,
};

fn catalog() -> StartupCatalog {
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias("ChatMessage", conduit_core::kind_id("chat/message@1"))
        .unwrap();
    catalog
        .insert(KindSignature {
            kind: "time/every".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "freq".into(),
                value_type: "Duration".into(),
                default: None,
            }],
        })
        .unwrap();
    catalog
        .insert(KindSignature {
            kind: "text/upper".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    catalog
        .insert(KindSignature {
            kind: "time/default".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "freq".into(),
                value_type: "Duration".into(),
                default: Some("1s".into()),
            }],
        })
        .unwrap();
    catalog
        .insert(KindSignature {
            kind: "pair/make".into(),
            startup_parameters: vec![
                StartupParameterSignature {
                    name: "left".into(),
                    value_type: "Text".into(),
                    default: None,
                },
                StartupParameterSignature {
                    name: "right".into(),
                    value_type: "Text".into(),
                    default: None,
                },
            ],
        })
        .unwrap();
    catalog
}

fn check(source: &str) -> crate::CheckedSyntaxDocument {
    let parsed = parse_syntax_document(source);
    check_syntax_document(&parsed, &catalog()).expect("canonical syntax checks")
}

const CODED_OUTCOME: &str =
    "type Outcome =\n    ready\n    | refused\n\ncode test/outcome = Outcome as u8\n";

#[test]
fn pre_v1_representation_keyword_is_not_a_compatibility_alias() {
    let parsed = parse_syntax_document(
        "type Outcome =\n    ready\n    | refused\n\nrepresentation test/outcome = Outcome as u8\n",
    );
    assert!(!parsed.diagnostics.is_empty());
}

#[test]
fn code_is_named_bounded_and_distinct_from_type_identity() {
    let checked = check(CODED_OUTCOME);
    let value_type = &checked.native_types[0];
    let code = &checked.codes[0];
    assert_eq!(code.name, "test/outcome");
    assert_eq!(code.value_type, value_type.identity);
    assert_ne!(code.compatibility_id, value_type.identity.as_str());
    assert_eq!(code.exact_bytes, 1);
    assert_eq!(code.maximum_bytes, 1);
    assert_eq!(code.maximum_decode_steps, 3);
    assert_eq!(code.invalid_refusal, crate::CheckedCodeRefusal::InvalidTag);
    assert_eq!(code.mappings[0].variant, "ready");
    assert_eq!(code.mappings[0].discriminant, 0);
}

#[test]
fn code_iota_may_start_at_an_explicit_u8_value() {
    let checked = check(
        "type Outcome =\n    ready\n    | refused\n\ncode test/outcome = Outcome as u8 from 1\n",
    );
    let mappings = &checked.codes[0].mappings;
    assert_eq!(mappings[0].discriminant, 1);
    assert_eq!(mappings[1].discriminant, 2);
}

#[test]
fn code_iota_refuses_u8_overflow() {
    let parsed = parse_syntax_document(
        "type Outcome =\n    ready\n    | refused\n\ncode test/outcome = Outcome as u8 from 255\n",
    );
    let error = check_syntax_document(&parsed, &catalog()).unwrap_err();
    assert!(error.message.contains("iota exceeds 255"));
}

#[test]
fn code_mapping_changes_compatibility_not_semantic_type_identity() {
    let first = check(CODED_OUTCOME);
    let second = check("type Outcome =\n    ready\n    | refused\n\ncode test/outcome = Outcome as u8\n    refused\n    ready\n");
    assert_eq!(
        first.native_types[0].identity,
        second.native_types[0].identity
    );
    assert_ne!(
        first.codes[0].compatibility_id,
        second.codes[0].compatibility_id
    );
}

#[test]
fn explicit_iota_order_requires_each_variant_exactly_once() {
    for (mapping, expected) in [
        ("    ready\n", "missing variant"),
        (
            "    ready\n    ready\n    refused\n",
            "mapped more than once",
        ),
        ("    ready\n    refused\n    imaginary\n", "unknown variant"),
    ] {
        let source = alloc::format!("{CODED_OUTCOME}{mapping}");
        let error = check_syntax_document(&parse_syntax_document(&source), &catalog()).unwrap_err();
        assert_eq!(error.code, "CND-FRM-059");
        assert!(error.message.contains(expected), "{}", error.message);
    }
}

#[test]
fn local_form_is_checked_through_one_private_ordinary_form_seam() {
    let checked = check("form outer {\n form helper {\n }\n child: helper\n}\n");
    assert_eq!(checked.forms.len(), 2);
    let outer = checked
        .forms
        .iter()
        .find(|form| form.name == "outer")
        .unwrap();
    let helper = checked
        .forms
        .iter()
        .find(|form| form.name.ends_with("/outer/helper"))
        .unwrap();
    assert_eq!(outer.gears[0].kind, helper.name);
    assert!(helper.name.starts_with("$local/"));
}

#[test]
fn local_form_privacy_and_implicit_capture_fail_closed() {
    let private = parse_syntax_document(
        "form owner {\n form helper {\n }\n child: helper\n}\n\nform thief {\n stolen: helper\n}\n",
    );
    let error = check_syntax_document(&private, &catalog()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-028");
    assert!(error.message.contains("helper"));

    let capture = parse_syntax_document(
        "form outer (\n limit: Count = 3\n >> value: Count\n) {\n form helper (\n  >> inner: Count\n  mapped: Count >>\n ) = (. + limit)\n child: helper\n}\n",
    );
    let error = check_syntax_document(&capture, &catalog()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-061");
    assert!(error.message.contains("cannot implicitly capture"));
    assert!(error.message.contains("limit"));
}

#[test]
fn local_form_captures_outer_type_parameters_only_as_exact_compile_time_identity() {
    let checked = check(
        "form outer (\n item: type\n >> value: item\n mapped: item >>\n) {\n form helper (\n  >> inner: item\n  mapped: item >>\n ) {\n  inner >> mapped\n }\n helper: helper\n value >> helper >> mapped\n}\n\nform main {\n child: outer(item = Count)\n}\n",
    );
    let outer = checked
        .forms
        .iter()
        .find(|form| form.name.starts_with("outer["))
        .unwrap();
    let helper = checked
        .forms
        .iter()
        .find(|form| form.name.starts_with("$local/outer/helper["))
        .unwrap();
    assert_eq!(outer.gears[0].kind, helper.name);
    assert!(helper.name.contains("item=value/count"));
}

fn text_port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id("value/text"),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

fn standard_glyph_catalog() -> StartupCatalog {
    let mut catalog = catalog();
    for kind in [
        "flow/merge",
        "flow/zip",
        "flow/race",
        "state/combine-latest",
        "current/sample",
    ] {
        catalog
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        if kind == "flow/merge" {
            catalog
                .insert_homogeneous_variadic_fore(
                    kind,
                    crate::HomogeneousVariadicFore::new(
                        text_port("operand", PortDirection::Input),
                        text_port("result", PortDirection::Output),
                        2,
                        16,
                    )
                    .unwrap(),
                )
                .unwrap();
        } else {
            catalog
                .insert_fore(
                    kind,
                    CheckedFront::new(
                        vec![],
                        vec![
                            text_port("left", PortDirection::Input),
                            text_port("right", PortDirection::Input),
                        ],
                        vec![text_port("result", PortDirection::Output)],
                        None,
                    ),
                )
                .unwrap();
        }
    }
    catalog
}

#[test]
fn completion_policy_is_exact_checked_meaning() {
    let live = check("form example {\n tick: time/every(1s)\n}\n");
    let finite = check("form example {\n tick: time/every(1s)\n}.\n");
    assert_eq!(live.forms[0].completion, crate::FormCompletionPolicy::Live);
    assert_eq!(
        finite.forms[0].completion,
        crate::FormCompletionPolicy::SemanticCompletion
    );
    assert_ne!(
        live.forms[0].checked_form_id,
        finite.forms[0].checked_form_id
    );
}

#[test]
fn authored_with_alias_resolves_to_canonical_kind_without_changing_checked_identity() {
    let direct = check("form example {\n tick: time/every(1s)\n}\n");
    let alias = check("with time/every as cadence\nform example {\n tick: cadence(1s)\n}\n");
    assert_eq!(alias.forms[0].gears[0].kind, "time/every");
    assert_eq!(
        alias.forms[0].checked_form_id,
        direct.forms[0].checked_form_id
    );
    assert_ne!(alias.source_document_id, direct.source_document_id);
}

#[test]
fn explicit_glyph_alias_lowers_to_the_same_ordinary_inline_gear() {
    let direct = check(
        "form transform (\n input: Text >> output: Text\n) {\n input >> output\n}\nform example (\n input: Text >> output: Text\n) {\n input >> transform() >> output\n}\n",
    );
    let glyph = check(
        "with transform as ^^\nform transform (\n input: Text >> output: Text\n) {\n input >> output\n}\nform example (\n input: Text >> output: Text\n) {\n input ^^ output\n}\n",
    );
    assert_eq!(glyph.forms[0].gears[0].kind, "transform");
    assert_eq!(glyph.forms[0].cords, direct.forms[0].cords);
    assert_eq!(
        glyph.forms[0].checked_form_id,
        direct.forms[0].checked_form_id
    );
    assert_ne!(glyph.source_document_id, direct.source_document_id);
}

#[test]
fn installed_kind_glyph_requires_and_uses_its_exact_checked_fore() {
    let source = "with text/upper as ^^\nform example (\n input: Text >> output: Text\n) {\n input ^^ output\n}\n";
    let missing = check_syntax_document(&parse_syntax_document(source), &catalog())
        .expect_err("a startup signature alone is not an exact runtime Fore");
    assert!(missing.message.contains("exact checked Fore"));

    let mut catalog = catalog();
    let input = text_port("input", PortDirection::Input);
    let output = text_port("output", PortDirection::Output);
    catalog
        .insert_fore(
            "text/upper",
            CheckedFront::new(
                vec![],
                vec![input.clone()],
                vec![output.clone()],
                Some((input.port_id, output.port_id)),
            ),
        )
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &catalog).unwrap();
    assert_eq!(checked.forms[0].gears[0].kind, "text/upper");
}

#[test]
fn configured_gear_occurrence_may_have_a_glyph_name() {
    let checked = check(
        "form transform (\n input: Text >> output: Text\n) {\n input >> output\n}\nform example (\n input: Text >> output: Text\n) {\n ^^: transform\n input ^^ output\n}\n",
    );
    assert_eq!(checked.forms[0].gears[0].name.as_deref(), Some("^^"));
    assert!(matches!(
        &checked.forms[0].cords[0].stages[1],
        crate::CheckedCordStage::Reference(name) if name == "^^"
    ));
}

#[test]
fn fixed_arity_relational_glyph_binds_operands_in_checked_fore_order() {
    let checked = check(
        "sans glyphs\nwith pair as &>\nform pair (\n >> left: Text\n >> right: Text\n result: Text >>\n) {\n}\nform example (\n >> a: Text\n >> b: Text\n paired: Text >>\n) {\n a &> b >> paired\n}\n",
    );
    let crate::CheckedCordStage::RelationalGear {
        operands,
        gear,
        input_ports,
        output_port,
    } = &checked.forms[0].cords[0].stages[0]
    else {
        panic!("expected exact relational Gear")
    };
    assert_eq!(operands, &["a", "b"]);
    assert_eq!(gear.kind, "pair");
    assert_eq!(input_ports, &["left", "right"]);
    assert_eq!(output_port, "result");
}

#[test]
fn standard_glyph_prelude_resolves_every_reviewed_binding_lazily() {
    let catalog = standard_glyph_catalog();
    for (glyph, kind) in [
        ("><", "flow/merge"),
        ("&>", "flow/zip"),
        ("?>", "flow/race"),
        ("<>", "state/combine-latest"),
        ("@", "current/sample"),
    ] {
        let source = format!(
            "form example (\n >> a: Text\n >> b: Text\n result: Text >>\n) {{\n a {glyph} b >> result\n}}\n"
        );
        let checked = check_syntax_document(&parse_syntax_document(&source), &catalog).unwrap();
        assert_eq!(checked.forms[0].gears[0].kind, kind);
        let expansion = &checked.source_sugar_expansions[0];
        assert_eq!(expansion.authored, glyph);
        assert_eq!(expansion.ordinary_kind, kind);
        assert_eq!(expansion.checked_form_id, checked.forms[0].checked_form_id);
        assert_eq!(expansion.source_span, parsed_glyph_span(&source, glyph));
        let expected_inputs = if kind == "flow/merge" {
            ["operand-01", "operand-02"]
        } else {
            ["left", "right"]
        };
        assert_eq!(expansion.input_ports, expected_inputs);
        assert_eq!(expansion.output_ports, ["result"]);
        assert_eq!(expansion.operand_bindings.len(), 2);
        assert!(expansion.canonical_replacement.is_none());
    }

    check_syntax_document(
        &parse_syntax_document("form unrelated {\n}\n"),
        &StartupCatalog::new(),
    )
    .expect("unused prelude bindings do not require installed Kinds");
}

fn parsed_glyph_span(source: &str, glyph: &str) -> crate::Span {
    let parsed = parse_syntax_document(source);
    let crate::BackStatement::Cord(cord) = &parsed.forms[0].back[0] else {
        panic!("expected glyph cord")
    };
    let crate::CordStage::RelationalGlyph { glyph: parsed, .. } = &cord.stages[0] else {
        panic!("expected relational glyph")
    };
    assert_eq!(parsed.text, glyph);
    parsed.span
}

#[test]
fn explicit_unary_glyph_inspection_uses_the_checked_fore_and_lossless_replacement() {
    let mut catalog = catalog();
    catalog
        .insert_fore(
            "text/upper",
            CheckedFront::new(
                vec![],
                vec![text_port("text", PortDirection::Input)],
                vec![text_port("upper", PortDirection::Output)],
                Some((port_id("text"), port_id("upper"))),
            ),
        )
        .unwrap();
    let source = "sans glyphs\nwith text/upper as ^^\nform example (\n >> input: Text\n output: Text >>\n) {\n input >> ^^ >> output\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &catalog).unwrap();
    let expansion = &checked.source_sugar_expansions[0];
    assert_eq!(expansion.authored, "^^");
    assert_eq!(expansion.ordinary_kind, "text/upper");
    assert_eq!(expansion.input_ports, ["text"]);
    assert_eq!(expansion.output_ports, ["upper"]);
    assert_eq!(
        expansion.canonical_replacement.as_deref(),
        Some("text/upper")
    );
    assert_eq!(checked.forms[0].gears[0].kind, expansion.ordinary_kind);
}

#[test]
fn repeated_variadic_glyph_specializes_one_exact_ordinary_gear() {
    let checked = check_syntax_document(
        &parse_syntax_document(
            "form example (\n >> a: Text\n >> b: Text\n >> c: Text\n result: Text >>\n) {\n a >< b >< c >> result\n}\n",
        ),
        &standard_glyph_catalog(),
    )
    .unwrap();
    let crate::CheckedCordStage::RelationalGear {
        operands,
        gear,
        input_ports,
        ..
    } = &checked.forms[0].cords[0].stages[0]
    else {
        panic!("expected one specialized relational Gear")
    };
    assert_eq!(operands, &["a", "b", "c"]);
    assert_eq!(gear.kind, "flow/merge");
    assert_eq!(input_ports, &["operand-01", "operand-02", "operand-03"]);
}

#[test]
fn variadic_glyph_refuses_an_implicit_unary_specialization() {
    let source = "form example (\n >> a: Text\n result: Text >>\n) {\n a >< result\n}\n";
    let error = check_syntax_document(&parse_syntax_document(source), &standard_glyph_catalog())
        .expect_err("a reviewed variadic Fore retains its minimum arity");
    assert!(error.message.contains("accepts 2..=16 inputs, not 1"));
}

#[test]
fn sans_glyphs_removes_only_the_standard_prelude() {
    let catalog = standard_glyph_catalog();
    let body =
        "form example (\n >> a: Text\n >> b: Text\n result: Text >>\n) {\n a &> b >> result\n}\n";
    let missing = check_syntax_document(
        &parse_syntax_document(&format!("sans glyphs\n{body}")),
        &catalog,
    )
    .expect_err("opt-out removes the standard binding");
    assert!(missing.message.contains("did not resolve in lexical scope"));

    let explicit = check_syntax_document(
        &parse_syntax_document(&format!("sans glyphs\nwith flow/zip as &>\n{body}")),
        &catalog,
    )
    .expect("an explicit glyph import remains legal after opt-out");
    assert_eq!(explicit.forms[0].gears[0].kind, "flow/zip");
    assert_eq!(explicit.source_sugar_expansions[0].authored, "&>");
    assert_eq!(
        explicit.source_sugar_expansions[0].ordinary_kind,
        "flow/zip"
    );

    let occupied = check_syntax_document(
        &parse_syntax_document(&format!("with flow/zip as &>\n{body}")),
        &catalog,
    )
    .expect_err("a standard glyph must be opted out before rebinding");
    assert!(occupied.message.contains("put 'sans glyphs'"));
}

#[test]
fn relational_glyph_refuses_wrong_arity_and_mixed_adjacent_meanings() {
    let wrong_arity = "sans glyphs\nwith pair as &>\nform pair (\n >> left: Text\n >> right: Text\n result: Text >>\n) {\n}\nform example (\n >> a: Text\n paired: Text >>\n) {\n a &> a &> a >> paired\n}\n";
    let error = check_syntax_document(&parse_syntax_document(wrong_arity), &catalog())
        .expect_err("fixed arity comes from the exact checked Fore");
    assert!(error.message.contains("supplies 3 operands"));

    let mixed = "sans glyphs\nwith pair as &>\nwith time/default as ?>\nform pair (\n >> left: Text\n >> right: Text\n result: Text >>\n) {\n}\nform example (\n >> a: Text\n >> b: Text\n >> c: Text\n paired: Text >>\n) {\n a &> b ?> c >> paired\n}\n";
    let parsed = parse_syntax_document(mixed);
    assert!(parsed.diagnostics[0]
        .message
        .contains("mixed adjacent glyphs require explicit grouping"));
}

#[test]
fn grouped_with_resolves_each_exact_installed_kind() {
    let checked = check(
        "with time/{every, default}\nform example {\n first: every(1s)\n second: default\n}\n",
    );
    assert_eq!(
        checked.forms[0]
            .gears
            .iter()
            .map(|gear| gear.kind.as_str())
            .collect::<Vec<_>>(),
        ["time/every", "time/default"]
    );
}

#[test]
fn text_pattern_refinement_enters_the_checked_fore_and_identity() {
    let source = "form code (\n >> value: Text <= 16B ~ /[A-Z]{2}[0-9]{4}/\n) {\n}\n";
    let checked = check(source);
    let location = conduit_core::FrontValueLocation::Input(conduit_core::port_id("value"));
    let contract = checked.forms[0]
        .runtime_front
        .value_contract(&location)
        .unwrap();
    assert_eq!(contract.validate(b"AB1234"), Ok(()));
    assert_eq!(
        contract.validate(b"Ab1234"),
        Err(conduit_core::ValueConstraintRefusal::TextPattern)
    );

    let different = check("form code (\n >> value: Text <= 16B ~ /[A-Z]{3}[0-9]{3}/\n) {\n}\n");
    assert_ne!(
        checked.forms[0].checked_form_id,
        different.forms[0].checked_form_id
    );
}

#[test]
fn unresolved_unused_duplicate_and_shadowing_imports_refuse() {
    for (source, expected) in [
        (
            "with missing/kind as absent\nform example {\n}\n",
            "does not resolve",
        ),
        (
            "with time/every as cadence\nform example {\n}\n",
            "unused with alias",
        ),
        (
            "with time/every as cadence\nwith time/default as cadence\nform example {\n tick: cadence\n}\n",
            "duplicate with alias",
        ),
        (
            "with time/every as cadence\nform example {\n cadence: time/default\n}\n",
            "shadows a with alias",
        ),
    ] {
        let error = check_syntax_document(&parse_syntax_document(source), &catalog()).unwrap_err();
        assert_eq!(error.code, "CND-FRM-056");
        assert!(error.message.contains(expected), "{}", error.message);
    }
}

#[test]
fn pattern_refinement_refuses_wrong_kind_and_bounds_star_by_the_text_envelope() {
    let wrong_kind = diagnostic("form code (\n >> value: Count ~ /[0-9]{1}/\n) {\n}\n");
    assert_eq!(wrong_kind.code, "CND-FRM-057");
    assert!(wrong_kind.message.contains("only canonical Text"));

    let checked = check("form code (\n >> value: Text <= 16B ~ /[A-Z]*/\n) {\n}\n");
    let [ValueConstraint::TextPattern { pattern, .. }] =
        checked.forms[0].runtime_front.value_contracts()[0]
            .contract
            .constraints
            .as_slice()
    else {
        panic!("expected one checked text-pattern refinement")
    };
    assert_eq!(pattern.maximum_input_characters, 16);
    assert!(pattern.maximum_match_steps <= MAX_PATTERN_MATCH_STEPS);
}

#[test]
fn keep_lifetime_optional_and_bound_are_exact_checked_meaning() {
    let mut catalog = StartupCatalog::new();
    catalog
        .insert(KindSignature {
            kind: "state/latest".into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    let check_keep = |declaration: &str| {
        let source = format!("form retained {{\n cell: {declaration}\n}}\n");
        check_syntax_document(&parse_syntax_document(&source), &catalog)
            .expect("canonical KEEP checks")
            .forms[0]
            .checked_form_id
            .clone()
    };
    let step = check_keep("keep Text");
    assert_ne!(step, check_keep("keep Text for this play"));
    assert_ne!(step, check_keep("keep Text?"));
    assert_ne!(step, check_keep("keep Text <= 128B"));
    assert_eq!(
        check_keep("keep Text for life"),
        check_keep("keep Text for this body")
    );
}

#[test]
fn optional_ports_resolve_to_the_canonical_finite_variant_profile() {
    let checked = check("form maybe (\n input: Scalar? >> output: $Scalar?\n) {\n}\n");
    let expected = conduit_core::optional_info_type(
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::SCALAR_INFO_ID))
            .unwrap(),
    )
    .unwrap();
    let expected_kind = expected.profile().unwrap().value_kind().clone();
    let front = checked.forms[0].checked_front();
    assert_eq!(front.inputs()[0].value_kind, expected_kind);
    assert_eq!(front.outputs()[0].value_kind, expected_kind);
    assert_eq!(checked.structured_type(&expected_kind), Some(&expected));
}

#[test]
fn finite_value_bounds_survive_in_the_exact_checked_fore() {
    let default =
        check("form bounded (\n title: Text\n >> input: Text\n output: Bytes >>\n) {\n}\n");
    let explicit = check(
        "form bounded (\n title: Text <= 256B\n >> input: Text <= 256B\n output: Bytes <= 64KiB >>\n) {\n}\n",
    );
    let narrower = check(
        "form bounded (\n title: Text <= 128B\n >> input: Text <= 128B\n output: Bytes <= 4KiB >>\n) {\n}\n",
    );
    let default_front = default.forms[0].checked_front();
    let explicit_front = explicit.forms[0].checked_front();
    let narrower_front = narrower.forms[0].checked_front();
    assert_eq!(default_front, explicit_front);
    assert_eq!(
        conduit_core::compute_checked_front_fingerprint(&default_front),
        conduit_core::compute_checked_front_fingerprint(&explicit_front)
    );
    assert_ne!(default_front, narrower_front);
    assert_ne!(
        conduit_core::compute_checked_front_fingerprint(&default_front),
        conduit_core::compute_checked_front_fingerprint(&narrower_front)
    );
    assert_eq!(
        default_front
            .value_contracts()
            .iter()
            .map(|value_contract| (
                value_contract.location.clone(),
                value_contract.contract.value_kind.clone(),
                value_contract.contract.maximum_bytes,
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                conduit_core::FrontValueLocation::Startup("title".into()),
                conduit_core::kind_id(conduit_core::TEXT_INFO_ID),
                256,
            ),
            (
                conduit_core::FrontValueLocation::Input(conduit_core::port_id("input")),
                conduit_core::kind_id(conduit_core::TEXT_INFO_ID),
                256,
            ),
            (
                conduit_core::FrontValueLocation::Output(conduit_core::port_id("output")),
                conduit_core::kind_id(conduit_core::BYTES_INFO_ID),
                65_536,
            ),
        ]
    );
}

#[test]
fn comment_only_edits_change_source_identity_but_not_checked_meaning() {
    let plain = check("form a {\n clock: time/every(1s)\n clock >> sink\n}\n");
    let commented = check(
        "# document note\nform a { # header\n clock: time/every(1s) # source\n clock >> sink # route\n} # close\n",
    );

    assert_ne!(plain.source_document_id, commented.source_document_id);
    assert_eq!(
        plain.forms[0].checked_form_id,
        commented.forms[0].checked_form_id
    );
    assert_eq!(plain.forms[0].gears, commented.forms[0].gears);
    assert_eq!(plain.forms[0].cords, commented.forms[0].cords);
}

#[test]
fn positional_named_and_local_reference_bindings_are_semantically_equivalent() {
    let positional = check("form a {\n clock: time/every(1s)\n}\n");
    let named = check("form a {\n clock: time/every(freq = 1s)\n}\n");
    let local = check("form a {\n freq = 1s\n clock: time/every(freq)\n}\n");

    assert_eq!(positional.forms[0].gears, named.forms[0].gears);
    assert_eq!(named.forms[0].gears, local.forms[0].gears);
    assert_eq!(
        positional.forms[0].checked_form_id,
        named.forms[0].checked_form_id
    );
    assert_eq!(
        named.forms[0].checked_form_id,
        local.forms[0].checked_form_id
    );
    assert_ne!(positional.source_document_id, named.source_document_id);
}

#[test]
fn local_values_and_gears_resolve_independently_of_statement_order() {
    let first = check("form a {\n freq = 1s\n clock: time/every(freq)\n clock >> sink\n}\n");
    let reordered = check("form a {\n clock >> sink\n clock: time/every(freq)\n freq = 1s\n}\n");

    assert_ne!(first.source_document_id, reordered.source_document_id);
    assert_eq!(
        first.forms[0].checked_form_id,
        reordered.forms[0].checked_form_id
    );
    assert_eq!(first.forms[0].gears, reordered.forms[0].gears);
}

#[test]
fn defaults_are_used_only_when_omitted_and_explicit_values_override_them() {
    let omitted = check("form a {\n clock: time/default\n}\n");
    let explicit = check("form a {\n clock: time/default(2s)\n}\n");
    let binding = &omitted.forms[0].gears[0].startup_bindings[0];

    assert_eq!(
        binding.value,
        CanonicalStartupValue::Quantity(conduit_core::Quantity::new(
            1,
            conduit_core::QuantityUnit::Second,
        ))
    );
    assert_eq!(
        explicit.forms[0].gears[0].startup_bindings[0].value,
        CanonicalStartupValue::Quantity(conduit_core::Quantity::new(
            2,
            conduit_core::QuantityUnit::Second,
        ))
    );
    assert_ne!(
        omitted.forms[0].checked_form_id,
        explicit.forms[0].checked_form_id
    );
}

#[test]
fn multiple_positional_and_named_bindings_follow_one_declared_signature() {
    let positional = check("form a {\n pair: pair/make(\"a\", \"b\")\n}\n");
    let named = check("form a {\n pair: pair/make(left = \"a\", right = \"b\")\n}\n");
    assert_eq!(positional.forms[0].gears, named.forms[0].gears);
    assert_eq!(
        positional.forms[0].checked_form_id,
        named.forms[0].checked_form_id
    );
}

#[test]
fn dependent_defaults_use_explicit_caller_binding_without_mutating_signature() {
    let mut catalog = catalog();
    catalog
        .insert(KindSignature {
            kind: "pair/default".into(),
            startup_parameters: vec![
                StartupParameterSignature {
                    name: "left".into(),
                    value_type: "Text".into(),
                    default: Some("\"default\"".into()),
                },
                StartupParameterSignature {
                    name: "right".into(),
                    value_type: "Text".into(),
                    default: Some("left".into()),
                },
            ],
        })
        .unwrap();
    let parsed = parse_syntax_document("form a {\n pair: pair/default(left = \"caller\")\n}\n");
    let checked = check_syntax_document(&parsed, &catalog).unwrap();
    assert_eq!(
        checked.forms[0].gears[0].startup_bindings[1].value,
        CanonicalStartupValue::Literal("\"caller\"".into())
    );
}

#[test]
fn forward_reference_chains_resolve_to_one_canonical_value() {
    let checked = check(
        "form a {\n first = second\n second = third\n third = 1s\n clock: time/every(first)\n}\n",
    );

    assert_eq!(
        checked.forms[0].gears[0].startup_bindings[0].value,
        CanonicalStartupValue::Quantity(conduit_core::Quantity::new(
            1,
            conduit_core::QuantityUnit::Second,
        ))
    );
}

#[test]
fn reusable_form_arguments_use_declared_front_startup_signature_without_expansion() {
    let checked =
        check("form badge (\n title: Text = \"Conduit\"\n) {\n}\n\nform page {\n hero: badge\n}\n");
    let page = checked
        .forms
        .iter()
        .find(|form| form.name == "page")
        .unwrap();

    assert_eq!(page.gears[0].kind, "badge");
    assert_eq!(
        page.gears[0].startup_bindings[0].value,
        CanonicalStartupValue::Literal("\"Conduit\"".into())
    );
}

#[test]
fn front_defaults_are_resolved_in_definition_scope_for_checked_identity() {
    let alias = check("form badge (\n title: Text = \"Conduit\"\n label: Text = title\n) {\n}\n");
    let literal =
        check("form badge (\n title: Text = \"Conduit\"\n label: Text = \"Conduit\"\n) {\n}\n");

    assert_eq!(
        alias.forms[0].startup_parameters[1].default,
        Some(CanonicalStartupValue::Literal("\"Conduit\"".into()))
    );
    assert_eq!(
        alias.forms[0].checked_form_id,
        literal.forms[0].checked_form_id
    );
}

fn diagnostic(source: &str) -> crate::SyntaxCheckDiagnostic {
    let parsed = parse_syntax_document(source);
    check_syntax_document(&parsed, &catalog()).expect_err("semantic source is rejected")
}

#[test]
fn duplicate_immutable_bindings_fail_without_last_write_wins() {
    let error = diagnostic("form a {\n freq = 1s\n freq = 2s\n}\n");
    assert_eq!(error.code, "CND-FRM-020");
    assert!(error.message.contains("duplicate immutable binding 'freq'"));
    assert!(error.message.contains("there is no later assignment"));
}

#[test]
fn duplicate_named_arguments_are_conflicting_not_last_write_wins() {
    let error = diagnostic("form a {\n clock: time/every(freq = 1s, freq = 2s)\n}\n");
    assert_eq!(error.code, "CND-FRM-021");
    assert!(error.message.contains("conflicting gear argument"));
}

#[test]
fn unknown_missing_and_excess_parameters_are_distinct() {
    let unknown = diagnostic("form a {\n clock: time/every(rate = 1s)\n}\n");
    let missing = diagnostic("form a {\n clock: time/every\n}\n");
    let excess = diagnostic("form a {\n clock: time/every(1s, 2s)\n}\n");

    assert_eq!(unknown.code, "CND-FRM-022");
    assert_eq!(missing.code, "CND-FRM-023");
    assert_eq!(excess.code, "CND-FRM-024");
}

#[test]
fn positional_and_named_binding_of_one_parameter_is_rejected() {
    let error = diagnostic("form a {\n clock: time/every(1s, freq = 2s)\n}\n");
    assert_eq!(error.code, "CND-FRM-025");
    assert!(error.message.contains("both bind 'freq'"));
}

#[test]
fn startup_dependency_cycles_are_rejected_exactly() {
    let error = diagnostic("form a {\n a = b\n b = a\n clock: time/every(a)\n}\n");
    assert_eq!(error.code, "CND-FRM-026");
    assert!(error.message.contains("dependency cycle"));
}

#[test]
fn front_default_cycles_are_rejected_even_before_invocation() {
    let error = diagnostic("form a (\n left: Text = right\n right: Text = left\n) {\n}\n");
    assert_eq!(error.code, "CND-FRM-026");
}

#[test]
fn runtime_ports_cannot_masquerade_as_startup_values() {
    let error = diagnostic("form a (\n >> freq: Duration\n) {\n clock: time/every(freq)\n}\n");
    assert_eq!(error.code, "CND-FRM-027");
    assert!(error.message.contains("runtime port 'freq'"));
}

#[test]
fn runtime_ports_hidden_inside_unsupported_expressions_still_fail_as_runtime_values() {
    let error =
        diagnostic("form a (\n >> freq: Duration\n) {\n clock: time/every(list(freq))\n}\n");
    assert_eq!(error.code, "CND-FRM-027");
}

#[test]
fn local_bindings_cannot_shadow_front_values_or_runtime_ports() {
    let parameter = diagnostic("form a (\n freq: Duration\n) {\n freq = 1s\n}\n");
    let runtime = diagnostic("form a (\n >> freq: Duration\n) {\n freq = 1s\n}\n");
    assert_eq!(parameter.code, "CND-FRM-020");
    assert_eq!(runtime.code, "CND-FRM-020");
}

#[test]
fn public_front_names_cannot_be_duplicated_or_shadowed_by_gears() {
    let duplicate = diagnostic("form a (\n >> value: Text\n >> value: Text\n) {\n}\n");
    let shadow = diagnostic("form a (\n >> clock: Duration\n) {\n clock: time/every(1s)\n}\n");
    assert_eq!(duplicate.code, "CND-FRM-050");
    assert_eq!(shadow.code, "CND-FRM-050");
    assert!(shadow.message.contains("ambiguously shadowed"));
}

#[test]
fn duplicate_gear_and_unsupported_kind_diagnostics_are_stable() {
    let duplicate = diagnostic("form a {\n clock: time/every(1s)\n clock: time/every(2s)\n}\n");
    let unsupported = diagnostic("form a {\n gear: unknown/op\n}\n");
    assert_eq!(duplicate.code, "CND-FRM-029");
    assert_eq!(unsupported.code, "CND-FRM-028");
}

#[test]
fn unsupported_expression_forms_fail_instead_of_becoming_opaque_literals() {
    let error = diagnostic("form a {\n clock: time/every(first + second)\n}\n");
    let compact = diagnostic("form a {\n clock: time/every(first+second)\n}\n");
    assert_eq!(error.code, "CND-FRM-030");
    assert_eq!(compact.code, "CND-FRM-030");
}

#[test]
fn shorthand_pair_participates_in_checked_identity() {
    let shorthand = check("form a (\n input: Tick >> output: Tick\n) {\n}\n");
    let auxiliary = check("form a (\n >> input: Tick\n output: Tick >>\n) {\n}\n");
    assert_ne!(
        shorthand.forms[0].checked_form_id,
        auxiliary.forms[0].checked_form_id
    );
}

#[test]
fn delimiter_like_literal_text_is_bound_unambiguously_into_identity() {
    let first = check("form a {\n clock: time/every(\"a:b|c\")\n}\n");
    let second = check("form a {\n clock: time/every(\"a:b|d\")\n}\n");
    assert_ne!(
        first.forms[0].checked_form_id,
        second.forms[0].checked_form_id
    );
}

#[test]
fn checked_front_equality_ignores_callable_name_and_back() {
    let checked = check(
        "form first (\n count: Count = 1\n input: Tick >> output: Tick\n) {\n}\n\nform second (\n count: Count = 2\n input: Tick >> output: Tick\n) {\n clock: time/every(1s)\n}\n",
    );
    assert_eq!(
        checked.forms[0].checked_front(),
        checked.forms[1].checked_front()
    );
    assert_ne!(
        checked.forms[0].checked_form_id,
        checked.forms[1].checked_form_id
    );
}

#[test]
fn startup_aliases_canonicalize_front_identity_and_fingerprint() {
    let mut aliases = catalog();
    aliases
        .insert_value_kind_alias("ShortText", conduit_core::kind_id("value/text"))
        .unwrap();
    aliases
        .insert_value_kind_alias("LongText", conduit_core::kind_id("value/text"))
        .unwrap();
    let short = parse_syntax_document("form a (\n value: ShortText\n) {\n}\n");
    let long = parse_syntax_document("form a (\n value: LongText\n) {\n}\n");
    let short = check_syntax_document(&short, &aliases).unwrap();
    let long = check_syntax_document(&long, &aliases).unwrap();
    let short_front = short.forms[0].checked_front();
    let long_front = long.forms[0].checked_front();

    assert_eq!(short_front, long_front);
    assert_eq!(
        short_front.startup_parameters()[0].value_type.as_str(),
        "value/text"
    );
    assert_eq!(
        conduit_core::compute_checked_front_fingerprint(&short_front),
        conduit_core::compute_checked_front_fingerprint(&long_front)
    );
}

#[test]
fn startup_type_and_default_semantics_have_explicit_identity_boundaries() {
    let text_default = check("form a (\n value: Text = \"one\"\n) {\n}\n");
    let other_default = check("form a (\n value: Text = \"two\"\n) {\n}\n");
    let count = check("form a (\n value: Count = 1\n) {\n}\n");

    assert_eq!(
        text_default.forms[0].checked_front(),
        other_default.forms[0].checked_front()
    );
    assert_ne!(
        text_default.forms[0].checked_form_id,
        other_default.forms[0].checked_form_id
    );
    assert_ne!(
        text_default.forms[0].checked_front(),
        count.forms[0].checked_front()
    );
    assert_ne!(
        conduit_core::compute_checked_front_fingerprint(&text_default.forms[0].checked_front()),
        conduit_core::compute_checked_front_fingerprint(&count.forms[0].checked_front())
    );
}

#[test]
fn fixed_width_integer_literals_are_range_checked_and_canonicalized() {
    let mut catalog = StartupCatalog::new();
    catalog
        .insert(KindSignature {
            kind: "system/register".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "U8".into(),
                default: None,
            }],
        })
        .unwrap();
    let checked = |literal: &str| {
        let source = format!("form register {{\n value: system/register({literal})\n}}\n");
        check_syntax_document(&parse_syntax_document(&source), &catalog)
    };

    let decimal = checked("255").unwrap();
    for equivalent in ["0xff", "0b1111_1111", "0o377"] {
        let checked = checked(equivalent).unwrap();
        assert_eq!(
            checked.forms[0].checked_form_id, decimal.forms[0].checked_form_id,
            "{equivalent}"
        );
        assert_eq!(
            checked.forms[0].gears[0].startup_bindings[0].value,
            CanonicalStartupValue::Literal("255".into())
        );
    }
    for refused in ["256", "-1", "0x_1", "0b2"] {
        let error = checked(refused).unwrap_err();
        assert_eq!(error.code, "CND-FRM-055", "{refused}");
    }
}

#[test]
fn fixed_width_integer_defaults_are_checked_before_identity() {
    let accepted = check_syntax_document(
        &parse_syntax_document("form accepted (\n value: I8 = -128\n) {\n}\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    assert_eq!(
        accepted.forms[0].startup_parameters[0].default,
        Some(CanonicalStartupValue::Literal("-128".into()))
    );
    let error = check_syntax_document(
        &parse_syntax_document("form refused (\n value: I8 = 128\n) {\n}\n"),
        &StartupCatalog::new(),
    )
    .unwrap_err();
    assert_eq!(error.code, "CND-FRM-055");
}

#[test]
fn native_scalar_record_and_variant_types_own_nominal_checked_identity() {
    let source = r#"type Note = U8 in 0..=127

type Position = {
    x: Distance
    y: Distance
}

type MusicEvent =
    note {
        velocity: U8 in 0..=127
        pitches: sequence Note <= 16
    }
    | rest

form perform (
    >> note: Note
    >> position: Position
    event: MusicEvent >>
) {
}
"#;
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();

    assert_eq!(checked.native_types.len(), 3);
    assert!(matches!(
        checked.native_types[0].value_type.shape(),
        conduit_core::StructuredInfoTypeShape::Nominal { .. }
    ));
    assert!(matches!(
        checked.native_types[1].value_type.shape(),
        conduit_core::StructuredInfoTypeShape::Record { .. }
    ));
    assert!(matches!(
        checked.native_types[2].value_type.shape(),
        conduit_core::StructuredInfoTypeShape::Variant { .. }
    ));
    assert_eq!(checked.native_types[0].value_contracts.len(), 1);
    assert_eq!(
        checked.native_types[2].value_contracts[0].representation_path,
        "|note.velocity"
    );
    let front = checked.forms[0].checked_front();
    assert_eq!(
        front.inputs()[0].value_kind,
        checked.native_types[0]
            .value_type
            .profile()
            .unwrap()
            .value_kind()
            .clone()
    );
    assert_eq!(
        front.outputs()[0].value_kind,
        checked.native_types[2]
            .value_type
            .profile()
            .unwrap()
            .value_kind()
            .clone()
    );
}

#[test]
fn native_record_where_laws_are_typed_owned_and_enforced() {
    let source = "type Interval = {\n    start: U32\n    end: U32\n    where .start <= .end\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let interval = &checked.native_types[0];
    assert_eq!(interval.invariants.len(), 1);

    let make = |start: u32, end: u32| {
        let conduit_core::StructuredInfoTypeShape::Record { fields, .. } =
            interval.value_type.shape()
        else {
            panic!("Interval is a record")
        };
        let values = fields
            .iter()
            .map(|field| {
                let value = if field.name() == "start" { start } else { end };
                let encoded = crate::rust_binding::primitive_into_structured(
                    field.value_type().clone(),
                    &value,
                )
                .unwrap();
                conduit_core::StructuredFieldValue::new(field.name(), encoded).unwrap()
            })
            .collect();
        conduit_core::StructuredInfoValue::record(interval.value_type.clone(), values).unwrap()
    };
    crate::rust_binding::validate_native_invariants(&make(4, 4), &interval.invariants).unwrap();
    assert_eq!(
        crate::rust_binding::validate_native_invariants(&make(5, 4), &interval.invariants),
        Err(crate::rust_binding::NativeBindingRefusal::ViolatedInvariant { index: 0 }),
        "{:#?}",
        interval.invariants[0]
    );

    let changed = check_syntax_document(
        &parse_syntax_document(
            "type Interval = {\n    start: U32\n    end: U32\n    where .start < .end\n}\n",
        ),
        &StartupCatalog::new(),
    )
    .unwrap();
    assert_ne!(interval.identity, changed.native_types[0].identity);
}

#[test]
fn native_record_where_laws_must_be_boolean_and_follow_fields() {
    for source in [
        "type Bad = {\n    value: U32\n    where .value + 1\n}\n",
        "type Bad = {\n    where true\n    value: U32\n}\n",
        "type Bad = {\n    value: U32\n    where .value > 0\n    where .value > 0\n}\n",
    ] {
        let parsed = parse_syntax_document(source);
        assert!(
            !parsed.diagnostics.is_empty()
                || check_syntax_document(&parsed, &StartupCatalog::new()).is_err()
        );
    }
}

#[test]
fn native_types_preserve_open_semantic_range_ends_for_every_numeric_family() {
    let checked = check(
        "type Positive = Count in 0..\n\
         type AtMostOne = Scalar in ..=1.000000\n\
         type Warm = Temperature in -273°C..\n\
         type Small = U32 in ..=100\n",
    );
    let constraints = checked
        .native_types
        .iter()
        .map(|value_type| &value_type.value_contracts[0].contract.constraints[0])
        .collect::<Vec<_>>();
    assert!(matches!(
        constraints[0],
        ValueConstraint::UnsignedRange {
            minimum: Some(0),
            maximum: None,
            ..
        }
    ));
    assert!(matches!(
        constraints[1],
        ValueConstraint::SignedRange {
            minimum: None,
            maximum: Some(1_000_000),
            ..
        }
    ));
    assert!(matches!(
        constraints[2],
        ValueConstraint::QuantityRange {
            minimum: Some(_),
            maximum: None,
            ..
        }
    ));
    assert!(matches!(
        constraints[3],
        ValueConstraint::FixedIntegerRange {
            minimum: None,
            maximum: Some(_),
            ..
        }
    ));
}

#[test]
fn native_type_identity_ignores_source_trivia_but_not_semantic_name() {
    let first = check_syntax_document(
        &parse_syntax_document("type Note = U8 in 0..=127\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    let spaced = check_syntax_document(
        &parse_syntax_document("\n\ntype Note = U8 in 0..=127\n\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    let renamed = check_syntax_document(
        &parse_syntax_document("type Velocity = U8 in 0..=127\n"),
        &StartupCatalog::new(),
    )
    .unwrap();

    assert_eq!(
        first.native_types[0].identity,
        spaced.native_types[0].identity
    );
    assert_ne!(
        first.native_types[0].identity,
        renamed.native_types[0].identity
    );
}

#[test]
fn native_text_bounds_and_negated_refinements_remain_checked_contracts() {
    let checked = check_syntax_document(
        &parse_syntax_document("type Code = Text <= 8B not in [\"root\", \"admin\"] !~ /root/\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    let contract = &checked.native_types[0].value_contracts[0].contract;
    assert_eq!(contract.maximum_bytes, 8);
    assert_eq!(contract.constraints.len(), 2);
    assert!(matches!(
        contract.constraints[0],
        conduit_core::ValueConstraint::CanonicalMembership { negated: true, .. }
    ));
    assert!(matches!(
        contract.constraints[1],
        conduit_core::ValueConstraint::TextPattern { negated: true, .. }
    ));
}

#[test]
fn native_types_resolve_forward_references_and_refuse_recursive_cycles() {
    let forward = check_syntax_document(
        &parse_syntax_document("type Phrase = sequence Note <= 4\ntype Note = U8 in 0..=127\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    assert_eq!(forward.native_types[0].name, "Phrase");
    assert_eq!(
        forward.native_types[0].value_contracts[0].representation_path,
        "[]"
    );

    let cycle = check_syntax_document(
        &parse_syntax_document("type Left = Right?\ntype Right = Left?\n"),
        &StartupCatalog::new(),
    )
    .unwrap_err();
    assert_eq!(cycle.code, "CND-FRM-058");
    assert!(cycle.message.contains("Left -> Right -> Left"));
}

#[test]
fn native_fixed_collection_checks_to_exact_structured_collection_truth() {
    let checked = check_syntax_document(
        &parse_syntax_document("type Quartet = collection Note = 4\ntype Note = U8 in 0..=127\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    assert!(matches!(
        checked.native_types[0].value_type.shape(),
        conduit_core::StructuredInfoTypeShape::Nominal { representation, .. }
            if matches!(
                representation.shape(),
                conduit_core::StructuredInfoTypeShape::Collection { length: 4, .. }
            )
    ));
}

#[test]
fn native_types_work_in_keeps_and_data_refs_without_structural_interchange() {
    let mut catalog = StartupCatalog::new();
    catalog
        .insert(KindSignature {
            kind: "state/latest".into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(
            "type Note = U8 in 0..=127\nform memory (\n    >> saved: &Note\n) {\n    cell: keep Note\n}\n",
        ),
        &catalog,
    )
    .unwrap();
    let retained = checked.forms[0].gears[0].retained.as_ref().unwrap();
    assert_eq!(retained.value_type, checked.native_types[0].value_type);
    assert_eq!(
        checked.forms[0].checked_front().inputs()[0].value_kind,
        conduit_core::data_reference_kind(
            checked.native_types[0]
                .value_type
                .profile()
                .unwrap()
                .value_kind()
        )
    );

    let initialized = check_syntax_document(
        &parse_syntax_document(
            "type Note = U8 in 0..=127\nform memory {\n    cell: keep Note(60)\n}\n",
        ),
        &catalog,
    )
    .unwrap();
    assert!(initialized.forms[0].gears[0]
        .retained
        .as_ref()
        .unwrap()
        .initial
        .is_some());
    let refused = check_syntax_document(
        &parse_syntax_document(
            "type Note = U8 in 0..=127\nform memory {\n    cell: keep Note(200)\n}\n",
        ),
        &catalog,
    )
    .unwrap_err();
    assert_eq!(refused.code, "CND-FRM-058");
    assert!(refused.message.contains("refinement refuses"));

    let mismatch = check_syntax_document(
        &parse_syntax_document(
            "type Note = U8 in 0..=127\ntype Velocity = U8 in 0..=127\nform wrong (\n    input: Note >> output: Velocity\n) {\n    input >> output\n}\n",
        ),
        &StartupCatalog::new(),
    )
    .unwrap_err();
    assert_eq!(mismatch.code, "CND-FRM-058");
    assert!(mismatch.message.contains("semantic Type mismatch"));
    assert!(mismatch.message.contains("codes are compatible"));
}

#[test]
fn pure_expression_identity_is_structural_and_ignores_parentheses_and_trivia() {
    let compact = check_syntax_document(
        &parse_syntax_document(
            "form transform (\n input: U8 >> output: U8\n) {\n input >> (. + 1 * 2) >> output\n}\n",
        ),
        &StartupCatalog::new(),
    )
    .unwrap();
    let spaced = check_syntax_document(
        &parse_syntax_document(
            "form transform (\n input: U8 >> output: U8\n) {\n input >> (((.) + (1 * 2))) >> output\n}\n",
        ),
        &StartupCatalog::new(),
    )
    .unwrap();
    assert_eq!(
        compact.forms[0].checked_form_id,
        spaced.forms[0].checked_form_id
    );
    assert!(matches!(
        compact.forms[0].cords[0].stages[1],
        crate::CheckedCordStage::PureExpression { .. }
    ));
}

#[test]
fn checked_front_equality_binds_startup_ports_and_shorthand() {
    let baseline = check("form a (\n count: Count = 1\n input: Tick >> output: Tick\n) {\n}\n");
    let required = check("form a (\n count: Count\n input: Tick >> output: Tick\n) {\n}\n");
    let renamed = check("form a (\n limit: Count = 1\n input: Tick >> output: Tick\n) {\n}\n");
    let auxiliary =
        check("form a (\n count: Count = 1\n >> input: Tick\n output: Tick >>\n) {\n}\n");
    let flow = check("form a (\n count: Count = 1\n input: Tick... >> output: Tick...\n) {\n}\n");
    let closing_flow =
        check("form a (\n count: Count = 1\n input: Tick...| >> output: Tick...|\n) {\n}\n");
    let current = check("form a (\n count: Count = 1\n input: $Tick >> output: $Tick\n) {\n}\n");
    for changed in [required, renamed, auxiliary, flow, closing_flow, current] {
        assert_ne!(
            baseline.forms[0].checked_front(),
            changed.forms[0].checked_front()
        );
    }
}

#[test]
fn checked_front_canonicalizes_runtime_port_declaration_order() {
    let first = check(
        "form a (\n >> alpha: Tick\n >> beta: Text\n omega: Text >>\n zeta: Tick >>\n) {\n}\n",
    );
    let reordered = check(
        "form renamed (\n zeta: Tick >>\n omega: Text >>\n >> beta: Text\n >> alpha: Tick\n) {\n}\n",
    );
    assert_eq!(
        first.forms[0].checked_front(),
        reordered.forms[0].checked_front()
    );
}

#[test]
fn pool_declaration_seals_member_front_and_bound_without_nominal_identity() {
    let first = check(
        "form chat/peer (\n recv: ChatMessage...| >> send: ChatMessage...|\n) {\n}\n\nform room {\n pool peers: chat/peer(size = 2)\n}\n",
    );
    let renamed = check(
        "form renamed/peer (\n recv: ChatMessage...| >> send: ChatMessage...|\n) {\n}\n\nform room {\n pool peers: renamed/peer(size = 2)\n}\n",
    );
    let first_room = first.forms.iter().find(|form| form.name == "room").unwrap();
    let renamed_room = renamed
        .forms
        .iter()
        .find(|form| form.name == "room")
        .unwrap();
    assert_eq!(
        first_room.pools[0].member_front,
        renamed_room.pools[0].member_front
    );
    assert_eq!(first_room.checked_form_id, renamed_room.checked_form_id);

    let larger = check(
        "form chat/peer (\n recv: ChatMessage...| >> send: ChatMessage...|\n) {\n}\n\nform room {\n pool peers: chat/peer(size = 3)\n}\n",
    );
    let larger_room = larger
        .forms
        .iter()
        .find(|form| form.name == "room")
        .unwrap();
    assert_ne!(first_room.checked_form_id, larger_room.checked_form_id);
}

#[test]
fn pool_member_must_be_declared_and_size_is_a_positive_finite_bound() {
    let unknown = diagnostic("form room {\n pool peers: missing/peer(size = 2)\n}\n");
    assert_eq!(unknown.code, "CND-FRM-028");

    let zero = crate::parse_syntax_document(
        "form chat/peer {\n}\n\nform room {\n pool peers: chat/peer(size = 0)\n}\n",
    );
    assert!(zero.forms().is_err());
    let overflow = crate::parse_syntax_document(
        "form chat/peer {\n}\n\nform room {\n pool peers: chat/peer(size = 65536)\n}\n",
    );
    assert!(overflow.forms().is_err());
}

#[test]
fn scientific_quantity_defaults_and_locals_are_checked_typed_values() {
    let checked = check(
        "form thermostat (\n target: Temperature = 21°C\n width: PixelCount = 640px\n) {\n distance = 3.2m\n angle = 90°\n}\n",
    );
    let form = &checked.forms[0];
    assert_eq!(
        form.startup_parameters[0].default,
        Some(CanonicalStartupValue::Quantity(
            conduit_core::Quantity::new(21, conduit_core::QuantityUnit::Celsius,)
        ))
    );
    assert_eq!(
        form.startup_parameters[1].default,
        Some(CanonicalStartupValue::Quantity(
            conduit_core::Quantity::new(640, conduit_core::QuantityUnit::Pixel,)
        ))
    );
    assert!(form.local_values.iter().any(|(name, value)| {
        name == "distance"
            && *value
                == CanonicalStartupValue::Quantity(conduit_core::Quantity::new(
                    3_200_000,
                    conduit_core::QuantityUnit::Micrometer,
                ))
    }));
}

#[test]
fn scientific_quantity_dimension_and_canonical_spelling_are_checked() {
    let wrong_dimension = diagnostic("form bad (\n target: Temperature = 12V\n) {\n}\n");
    assert_eq!(wrong_dimension.code, "CND-FRM-055");

    let near_miss = diagnostic("form bad (\n target: Temperature = 21C\n) {\n}\n");
    assert_eq!(near_miss.code, "CND-FRM-055");
    assert!(near_miss.message.contains("use '°C'"));
}
