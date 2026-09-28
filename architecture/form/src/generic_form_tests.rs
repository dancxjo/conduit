use crate::{check_syntax_document, parse_syntax_document, CheckedSyntaxDocument, StartupCatalog};

fn check(source: &str) -> CheckedSyntaxDocument {
    let parsed = parse_syntax_document(source);
    check_syntax_document(&parsed, &StartupCatalog::new()).expect("generic source checks")
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
    let error = check_syntax_document(&parsed, &StartupCatalog::new())
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
    let error = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap_err();
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
    let error = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-057");
    assert!(error.message.contains("exact checked type"));
}

#[test]
fn specialization_expands_without_a_generic_runtime_gear() {
    let port = |name: &str, direction| conduit_core::PortDescriptor {
        port_id: conduit_core::port_id(name),
        value_kind: conduit_core::kind_id("value/text"),
        direction,
        temporal: conduit_core::PortTemporal::Value,
        abnormal_kind: None,
    };
    let mut profiles = crate::ProfileCatalog::new();
    profiles
        .insert(crate::KindProjection {
            kind_id: conduit_core::kind_id("test/pass"),
            kind_contract_revision: conduit_core::KindIdentity::from("test/pass@1"),
            inputs: vec![port("input", conduit_core::PortDirection::Input)],
            outputs: vec![port("output", conduit_core::PortDirection::Output)],
            configuration: vec![],
        })
        .unwrap();
    let parsed = parse_syntax_document(
        "form identity (\n item: type\n >> value: item\n result: item >>\n) {\n pass: test/pass\n value >> pass.input\n pass.output >> result\n}\n\
         form main (\n >> value: Text\n result: Text >>\n) {\n copy: identity(item = Text)\n value >> copy.value\n copy.result >> result\n}\n",
    );
    let checked = check_syntax_document(&parsed, &profiles.startup_catalog().unwrap()).unwrap();
    let authoring =
        crate::expand_canonical_form_for_authoring(&checked, "main", &profiles).unwrap();
    assert_eq!(authoring.expanded.gears.len(), 1);
    assert_eq!(authoring.expanded.gears[0].kind_id.as_str(), "test/pass");
    assert_eq!(authoring.input_bindings.len(), 1);
    assert_eq!(authoring.output_bindings.len(), 1);
    assert!(authoring
        .expanded
        .provenance
        .iter()
        .all(|entry| !entry.source_gear.contains("generic")));
}

#[test]
fn runtime_startup_values_remain_after_compile_time_type_arguments_disappear() {
    let checked = check(
        "form repeat (\n item: type\n copies: Count = 1\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         form main {\n repeated: repeat(item = Text, copies = 2)\n}\n",
    );
    let main = checked
        .forms
        .iter()
        .find(|form| form.name == "main")
        .unwrap();
    assert_eq!(main.gears[0].startup_parameters.len(), 1);
    assert_eq!(main.gears[0].startup_parameters[0].name, "copies");
    assert_eq!(main.gears[0].startup_bindings.len(), 1);
    assert_eq!(main.gears[0].startup_bindings[0].name, "copies");
    assert_eq!(
        main.gears[0].kind, "repeat[item=value/text]",
        "the compile-time argument survives only in exact specialization identity"
    );
}
