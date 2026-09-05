use crate::{
    check_syntax_document, parse_syntax_document, CanonicalStartupValue, KindSignature,
    StartupCatalog, StartupParameterSignature,
};

fn catalog() -> StartupCatalog {
    let mut catalog = StartupCatalog::new();
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

#[test]
fn comment_only_edits_change_source_identity_but_not_checked_meaning() {
    let plain = check("form a {\n clock: time/every(1s)\n clock > sink\n}\n");
    let commented = check(
        "# document note\nform a { # header\n clock: time/every(1s) # source\n clock > sink # route\n} # close\n",
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
    let first = check("form a {\n freq = 1s\n clock: time/every(freq)\n clock > sink\n}\n");
    let reordered = check("form a {\n clock > sink\n clock: time/every(freq)\n freq = 1s\n}\n");

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

    assert_eq!(binding.value, CanonicalStartupValue::Literal("1s".into()));
    assert_eq!(
        explicit.forms[0].gears[0].startup_bindings[0].value,
        CanonicalStartupValue::Literal("2s".into())
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
        CanonicalStartupValue::Literal("1s".into())
    );
}

#[test]
fn reusable_form_arguments_use_declared_face_startup_signature_without_expansion() {
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
fn face_defaults_are_resolved_in_definition_scope_for_checked_identity() {
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
fn face_default_cycles_are_rejected_even_before_invocation() {
    let error = diagnostic("form a (\n left: Text = right\n right: Text = left\n) {\n}\n");
    assert_eq!(error.code, "CND-FRM-026");
}

#[test]
fn runtime_ports_cannot_masquerade_as_startup_values() {
    let error = diagnostic("form a (\n > freq: Duration\n) {\n clock: time/every(freq)\n}\n");
    assert_eq!(error.code, "CND-FRM-027");
    assert!(error.message.contains("runtime port 'freq'"));
}

#[test]
fn runtime_ports_hidden_inside_unsupported_expressions_still_fail_as_runtime_values() {
    let error = diagnostic("form a (\n > freq: Duration\n) {\n clock: time/every(list(freq))\n}\n");
    assert_eq!(error.code, "CND-FRM-027");
}

#[test]
fn local_bindings_cannot_shadow_face_values_or_runtime_ports() {
    let parameter = diagnostic("form a (\n freq: Duration\n) {\n freq = 1s\n}\n");
    let runtime = diagnostic("form a (\n > freq: Duration\n) {\n freq = 1s\n}\n");
    assert_eq!(parameter.code, "CND-FRM-020");
    assert_eq!(runtime.code, "CND-FRM-020");
}

#[test]
fn public_face_names_cannot_be_duplicated_or_shadowed_by_gears() {
    let duplicate = diagnostic("form a (\n > value: Text\n > value: Text\n) {\n}\n");
    let shadow = diagnostic("form a (\n > clock: Duration\n) {\n clock: time/every(1s)\n}\n");
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
    let shorthand = check("form a (\n input: Tick > output: Tick\n) {\n}\n");
    let auxiliary = check("form a (\n > input: Tick\n output: Tick >\n) {\n}\n");
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
fn checked_face_equality_ignores_callable_name_and_back() {
    let checked = check(
        "form first (\n count: Count = 1\n input: Tick > output: Tick\n) {\n}\n\nform second (\n count: Count = 2\n input: Tick > output: Tick\n) {\n clock: time/every(1s)\n}\n",
    );
    assert_eq!(
        checked.forms[0].checked_face(),
        checked.forms[1].checked_face()
    );
    assert_ne!(
        checked.forms[0].checked_form_id,
        checked.forms[1].checked_form_id
    );
}

#[test]
fn checked_face_equality_binds_startup_ports_and_shorthand() {
    let baseline = check("form a (\n count: Count = 1\n input: Tick > output: Tick\n) {\n}\n");
    let required = check("form a (\n count: Count\n input: Tick > output: Tick\n) {\n}\n");
    let renamed = check("form a (\n limit: Count = 1\n input: Tick > output: Tick\n) {\n}\n");
    let auxiliary = check("form a (\n count: Count = 1\n > input: Tick\n output: Tick >\n) {\n}\n");
    let flow = check("form a (\n count: Count = 1\n input: Tick... > output: Tick...\n) {\n}\n");
    let closing_flow =
        check("form a (\n count: Count = 1\n input: Tick...| > output: Tick...|\n) {\n}\n");
    let current = check("form a (\n count: Count = 1\n input: $Tick > output: $Tick\n) {\n}\n");
    for changed in [required, renamed, auxiliary, flow, closing_flow, current] {
        assert_ne!(
            baseline.forms[0].checked_face(),
            changed.forms[0].checked_face()
        );
    }
}

#[test]
fn checked_face_canonicalizes_runtime_port_declaration_order() {
    let first =
        check("form a (\n > alpha: Tick\n > beta: Text\n omega: Text >\n zeta: Tick >\n) {\n}\n");
    let reordered = check(
        "form renamed (\n zeta: Tick >\n omega: Text >\n > beta: Text\n > alpha: Tick\n) {\n}\n",
    );
    assert_eq!(
        first.forms[0].checked_face(),
        reordered.forms[0].checked_face()
    );
}

#[test]
fn pool_declaration_seals_member_face_and_bound_without_nominal_identity() {
    let first = check(
        "form chat/peer (\n recv: ChatMessage...| > send: ChatMessage...|\n) {\n}\n\nform room {\n pool peers: chat/peer(size = 2)\n}\n",
    );
    let renamed = check(
        "form renamed/peer (\n recv: ChatMessage...| > send: ChatMessage...|\n) {\n}\n\nform room {\n pool peers: renamed/peer(size = 2)\n}\n",
    );
    let first_room = first.forms.iter().find(|form| form.name == "room").unwrap();
    let renamed_room = renamed
        .forms
        .iter()
        .find(|form| form.name == "room")
        .unwrap();
    assert_eq!(
        first_room.pools[0].member_face,
        renamed_room.pools[0].member_face
    );
    assert_eq!(first_room.checked_form_id, renamed_room.checked_form_id);

    let larger = check(
        "form chat/peer (\n recv: ChatMessage...| > send: ChatMessage...|\n) {\n}\n\nform room {\n pool peers: chat/peer(size = 3)\n}\n",
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
