use crate::{
    check_syntax_document, parse_syntax_document, CanonicalStartupValue, KindSignature,
    StartupCatalog, StartupParameterSignature,
};
use alloc::vec::Vec;
use conduit_core::{kind_id, port_id, CheckedFront, PortDescriptor, PortDirection, PortTemporal};

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
fn authored_use_alias_resolves_to_canonical_kind_without_changing_checked_identity() {
    let direct = check("form example {\n tick: time/every(1s)\n}\n");
    let alias = check("use time/every as cadence\nform example {\n tick: cadence(1s)\n}\n");
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
        "use transform as ^^\nform transform (\n input: Text >> output: Text\n) {\n input >> output\n}\nform example (\n input: Text >> output: Text\n) {\n input ^^ output\n}\n",
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
    let source = "use text/upper as ^^\nform example (\n input: Text >> output: Text\n) {\n input ^^ output\n}\n";
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
        "without glyphs\nuse pair as &>\nform pair (\n >> left: Text\n >> right: Text\n result: Text >>\n) {\n}\nform example (\n >> a: Text\n >> b: Text\n paired: Text >>\n) {\n a &> b >> paired\n}\n",
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
    }

    check_syntax_document(
        &parse_syntax_document("form unrelated {\n}\n"),
        &StartupCatalog::new(),
    )
    .expect("unused prelude bindings do not require installed Kinds");
}

#[test]
fn without_glyphs_removes_only_the_standard_prelude() {
    let catalog = standard_glyph_catalog();
    let body =
        "form example (\n >> a: Text\n >> b: Text\n result: Text >>\n) {\n a &> b >> result\n}\n";
    let missing = check_syntax_document(
        &parse_syntax_document(&format!("without glyphs\n{body}")),
        &catalog,
    )
    .expect_err("opt-out removes the standard binding");
    assert!(missing.message.contains("did not resolve in lexical scope"));

    let explicit = check_syntax_document(
        &parse_syntax_document(&format!("without glyphs\nuse flow/zip as &>\n{body}")),
        &catalog,
    )
    .expect("an explicit glyph import remains legal after opt-out");
    assert_eq!(explicit.forms[0].gears[0].kind, "flow/zip");

    let occupied = check_syntax_document(
        &parse_syntax_document(&format!("use flow/zip as &>\n{body}")),
        &catalog,
    )
    .expect_err("a standard glyph must be opted out before rebinding");
    assert!(occupied.message.contains("use 'without glyphs'"));
}

#[test]
fn relational_glyph_refuses_wrong_arity_and_mixed_adjacent_meanings() {
    let wrong_arity = "without glyphs\nuse pair as &>\nform pair (\n >> left: Text\n >> right: Text\n result: Text >>\n) {\n}\nform example (\n >> a: Text\n paired: Text >>\n) {\n a &> a &> a >> paired\n}\n";
    let error = check_syntax_document(&parse_syntax_document(wrong_arity), &catalog())
        .expect_err("fixed arity comes from the exact checked Fore");
    assert!(error.message.contains("supplies 3 operands"));

    let mixed = "without glyphs\nuse pair as &>\nuse time/default as ?>\nform pair (\n >> left: Text\n >> right: Text\n result: Text >>\n) {\n}\nform example (\n >> a: Text\n >> b: Text\n >> c: Text\n paired: Text >>\n) {\n a &> b ?> c >> paired\n}\n";
    let parsed = parse_syntax_document(mixed);
    assert!(parsed.diagnostics[0]
        .message
        .contains("mixed adjacent glyphs require explicit grouping"));
}

#[test]
fn named_type_parameters_specialize_to_exact_ordinary_fores() {
    let checked = check(
        "form identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         form text-identity (\n >> value: Text\n result: Text >>\n) {\n value >> identity(item = Text) >> result\n}\n\
         form bytes-identity (\n >> value: Bytes\n result: Bytes >>\n) {\n value >> identity(item = Bytes) >> result\n}\n",
    );

    assert!(checked.forms.iter().all(|form| form.name != "identity"));
    let text = checked
        .forms
        .iter()
        .find(|form| form.name == "identity[item=value/text]")
        .expect("Text specialization is retained as an exact ordinary Form");
    assert!(text.startup_parameters.is_empty());
    assert_eq!(
        text.runtime_front.inputs()[0].value_kind,
        conduit_core::kind_id("value/text")
    );
    assert_eq!(
        text.runtime_front.outputs()[0].value_kind,
        conduit_core::kind_id("value/text")
    );
    let text_input = conduit_core::FrontValueLocation::Input(conduit_core::port_id("value"));
    assert_eq!(
        text.runtime_front
            .value_contract(&text_input)
            .unwrap()
            .maximum_bytes,
        256
    );

    let bytes = checked
        .forms
        .iter()
        .find(|form| form.name == "identity[item=value/bytes]")
        .expect("Bytes specialization is retained as an exact ordinary Form");
    assert_eq!(
        bytes.runtime_front.inputs()[0].value_kind,
        conduit_core::kind_id("value/bytes")
    );
    assert_ne!(text.checked_form_id, bytes.checked_form_id);

    for wrapper in ["text-identity", "bytes-identity"] {
        let form = checked
            .forms
            .iter()
            .find(|form| form.name == wrapper)
            .unwrap();
        assert!(form.gears[0].startup_bindings.is_empty());
        assert!(form.gears[0].kind.starts_with("identity[item=value/"));
    }
}

#[test]
fn explicit_generic_application_cannot_disagree_with_connected_ports() {
    let parsed = parse_syntax_document(
        "form identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         form bad (\n >> value: Text\n result: Text >>\n) {\n value >> identity(item = Bytes) >> result\n}\n",
    );
    let error = check_syntax_document(&parsed, &catalog())
        .expect_err("Text cannot satisfy a Bytes specialization");
    assert_eq!(error.code, "CND-FRM-057");
    assert!(error.message.contains("connected ports require"));
}

#[test]
fn unary_generic_application_infers_one_unambiguous_port_type() {
    let checked = check(
        "form identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         form main (\n >> value: Text\n result: Text >>\n) {\n value >> identity() >> result\n}\n",
    );
    let main = checked
        .forms
        .iter()
        .find(|form| form.name == "main")
        .unwrap();
    assert_eq!(main.gears[0].kind, "identity[item=value/text]");
    assert!(main.gears[0].startup_bindings.is_empty());
}

#[test]
fn generic_inference_refuses_conflicting_port_evidence() {
    let parsed = parse_syntax_document(
        "form identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         form bad (\n >> value: Text\n result: Bytes >>\n) {\n value >> identity() >> result\n}\n",
    );
    let error = check_syntax_document(&parsed, &catalog()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-057");
    assert!(error.message.contains("conflicting types"));
}

#[test]
fn generic_specialization_preserves_temporal_modalities_and_data_references() {
    let checked = check(
        "form latest (\n item: type\n >> values: item...\n >> source: $item\n current: $item >>\n snapshot: &item >>\n) {\n source >> current\n}\n\
         form main {\n selected: latest(item = Text)\n}\n",
    );
    let latest = checked
        .forms
        .iter()
        .find(|form| form.name == "latest[item=value/text]")
        .unwrap();
    let values = latest
        .runtime_front
        .inputs()
        .iter()
        .find(|port| port.port_id.as_str() == "values")
        .unwrap();
    assert_eq!(
        values.temporal,
        conduit_core::PortTemporal::Flow { closes: false }
    );
    let source = latest
        .runtime_front
        .inputs()
        .iter()
        .find(|port| port.port_id.as_str() == "source")
        .unwrap();
    assert_eq!(source.temporal, conduit_core::PortTemporal::Current);
    let snapshot = latest
        .runtime_front
        .outputs()
        .iter()
        .find(|port| port.port_id.as_str() == "snapshot")
        .unwrap();
    assert_eq!(
        snapshot.value_kind,
        conduit_core::data_reference_kind(&conduit_core::kind_id("value/text"))
    );
}

#[test]
fn generic_use_alias_preserves_the_canonical_specialization_identity() {
    let template = "form library/identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n";
    let direct = check(&format!(
        "{template}form main (\n >> value: Text\n result: Text >>\n) {{\n value >> library/identity(item = Text) >> result\n}}\n"
    ));
    let aliased = check(&format!(
        "use library/identity as copy\n{template}form main (\n >> value: Text\n result: Text >>\n) {{\n value >> copy(item = Text) >> result\n}}\n"
    ));
    let direct_main = direct
        .forms
        .iter()
        .find(|form| form.name == "main")
        .unwrap();
    let aliased_main = aliased
        .forms
        .iter()
        .find(|form| form.name == "main")
        .unwrap();
    assert_eq!(direct_main.checked_form_id, aliased_main.checked_form_id);
    assert_eq!(
        direct_main.gears[0].kind,
        "library/identity[item=value/text]"
    );
    assert_eq!(direct_main.gears[0].kind, aliased_main.gears[0].kind);
}

#[test]
fn type_arguments_must_resolve_to_exact_checked_types() {
    let parsed = parse_syntax_document(
        "form identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         form bad {\n value: identity(item = Mystery)\n}\n",
    );
    let error = check_syntax_document(&parsed, &catalog()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-057");
    assert!(error.message.contains("exact checked type"));
}

#[test]
fn grouped_uses_resolve_each_exact_installed_kind() {
    let checked = check(
        "use time/{every, default}\nform example {\n first: every(1s)\n second: default\n}\n",
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
    let source =
        "form code (\n >> value: Text <= 16B where pattern(r\"[A-Z]{2}[0-9]{4}\")\n) {\n}\n";
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

    let different =
        check("form code (\n >> value: Text <= 16B where pattern(r\"[A-Z]{3}[0-9]{3}\")\n) {\n}\n");
    assert_ne!(
        checked.forms[0].checked_form_id,
        different.forms[0].checked_form_id
    );
}

#[test]
fn unresolved_unused_duplicate_and_shadowing_uses_refuse() {
    for (source, expected) in [
        (
            "use missing/kind as absent\nform example {\n}\n",
            "does not resolve",
        ),
        (
            "use time/every as cadence\nform example {\n}\n",
            "unused use alias",
        ),
        (
            "use time/every as cadence\nuse time/default as cadence\nform example {\n tick: cadence\n}\n",
            "duplicate use alias",
        ),
        (
            "use time/every as cadence\nform example {\n cadence: time/default\n}\n",
            "shadows a use alias",
        ),
    ] {
        let error = check_syntax_document(&parse_syntax_document(source), &catalog()).unwrap_err();
        assert_eq!(error.code, "CND-FRM-056");
        assert!(error.message.contains(expected), "{}", error.message);
    }
}

#[test]
fn pattern_refinement_refuses_wrong_kind_and_unbounded_source() {
    let wrong_kind =
        diagnostic("form code (\n >> value: Count where pattern(r\"[0-9]{1}\")\n) {\n}\n");
    assert_eq!(wrong_kind.code, "CND-FRM-057");
    assert!(wrong_kind.message.contains("only canonical Text"));

    let unbounded =
        diagnostic("form code (\n >> value: Text <= 16B where pattern(r\"[A-Z]*\")\n) {\n}\n");
    assert_eq!(unbounded.code, "CND-FRM-057");
    assert!(unbounded.message.contains("UnboundedRepeat"));
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
