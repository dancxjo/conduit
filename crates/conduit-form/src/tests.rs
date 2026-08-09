use super::{
    parse, parse_document, CompositeFaceTerminal, ConfigurationField, ConfigurationRule, FormError,
    KindDefinition, ProfileCatalog, MAXIMUM_FORM_NESTING_DEPTH, MAXIMUM_FORM_SOURCE_BYTES,
    MAXIMUM_FORM_TOKENS,
};
use conduit_core::{
    kind_id, port_id, CapabilityId, ConfigurationValue, KindContractRevision, PortDescriptor,
    PortDirection,
};

fn catalog() -> ProfileCatalog {
    catalog_with_source_contract("test/source@1", "out", "test/value")
}

fn catalog_with_source_contract(
    source_revision: &str,
    source_port: &str,
    value_kind: &str,
) -> ProfileCatalog {
    let value_kind = kind_id(value_kind);
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id("test/source"),
            kind_contract_revision: KindContractRevision::from(source_revision),
            inputs: Vec::new(),
            outputs: vec![PortDescriptor {
                port_id: port_id(source_port),
                value_kind: value_kind.clone(),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Value,
            }],
            configuration: vec![ConfigurationField {
                key: "count".to_string(),
                default_value: ConfigurationValue::U64(1),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: 4,
                },
            }],
        })
        .expect("source kind installs");
    catalog
        .insert(KindDefinition {
            kind_id: kind_id("test/sink"),
            kind_contract_revision: KindContractRevision::from("test/sink@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("in"),
                value_kind,
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Value,
            }],
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("sink kind installs");
    catalog
}

fn multi_value_catalog() -> ProfileCatalog {
    let mut catalog = catalog();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id("test/source-b"),
            kind_contract_revision: KindContractRevision::from("test/source-b@1"),
            inputs: Vec::new(),
            outputs: vec![PortDescriptor {
                port_id: port_id("bytes"),
                value_kind: kind_id("test/bytes"),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Value,
            }],
            configuration: Vec::new(),
        })
        .expect("second source installs");
    catalog
        .insert(KindDefinition {
            kind_id: kind_id("test/sink-b"),
            kind_contract_revision: KindContractRevision::from("test/sink-b@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("bytes"),
                value_kind: kind_id("test/bytes"),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Value,
            }],
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("second sink installs");
    catalog
}

#[test]
fn checked_form_identity_binds_contract_revision_and_ports() {
    let source =
        "form 0\n\nidentity {\n source: test/source\n sink: test/sink\n source > sink\n}\n";
    let baseline = parse(source, &catalog()).expect("baseline parses");
    let revised = parse(
        source,
        &catalog_with_source_contract("test/source@2", "out", "test/value"),
    )
    .expect("revised contract parses");
    let renamed_port = parse(
        source,
        &catalog_with_source_contract("test/source@1", "renamed", "test/value"),
    )
    .expect("renamed port parses");
    let retyped_port = parse(
        source,
        &catalog_with_source_contract("test/source@1", "out", "test/value-v2"),
    )
    .expect("retyped port parses");

    assert_ne!(baseline.checked_form_id, revised.checked_form_id);
    assert_ne!(baseline.checked_form_id, renamed_port.checked_form_id);
    assert_ne!(baseline.checked_form_id, retyped_port.checked_form_id);
    assert_eq!(baseline.source_document_id, revised.source_document_id);
    assert_ne!(baseline.expanded_form_id, revised.expanded_form_id);
    assert_ne!(baseline.expanded_form_id, renamed_port.expanded_form_id);
    assert_ne!(baseline.expanded_form_id, retyped_port.expanded_form_id);
}

#[test]
fn source_checked_and_expanded_form_identities_stay_distinct() {
    let baseline_source =
        "form 0\n\nidentity {\n source: test/source\n sink: test/sink\n source > sink\n}\n";
    let spelling_only_source = "# author note\nform 0\nidentity {\n\n source: test/source\n sink: test/sink\n source > sink\n}\n";
    let semantic_change_source = "form 0\n\nidentity {\n source: test/source\n sink: test/sink\n source.count = 2\n source > sink\n}\n";

    let baseline = parse(baseline_source, &catalog()).expect("baseline parses");
    let spelling_only = parse(spelling_only_source, &catalog()).expect("spelling-only edit parses");
    let semantic_change = parse(semantic_change_source, &catalog()).expect("semantic edit parses");

    assert_ne!(
        baseline.source_document_id,
        spelling_only.source_document_id
    );
    assert_eq!(baseline.checked_form_id, spelling_only.checked_form_id);
    assert_eq!(baseline.expanded_form_id, spelling_only.expanded_form_id);

    assert_ne!(
        baseline.source_document_id,
        semantic_change.source_document_id
    );
    assert_ne!(baseline.checked_form_id, semantic_change.checked_form_id);
    assert_ne!(baseline.expanded_form_id, semantic_change.expanded_form_id);
}

#[test]
fn lossless_document_round_trips_utf8_comments_and_layout() {
    let source = "# café\r\nform 0\n\n  δemo {  \n source: test/source\n sink: test/sink\n source > sink\n}\n";
    let document = parse_document(source, &catalog());
    let checked = document.checked().expect("document checks");
    let compatibility = parse(source, &catalog()).expect("compatibility parser checks");

    assert_eq!(document.round_trip(), source);
    assert_eq!(
        document
            .tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect::<String>(),
        source
    );
    for token in &document.tokens {
        assert_eq!(
            source.get(token.span.start..token.span.end),
            Some(token.text.as_str())
        );
    }
    assert_eq!(checked, &compatibility);
    assert!(document.diagnostics.is_empty());
}

#[test]
fn lossless_document_retains_later_source_after_recoverable_error() {
    let source = "form 0\nbroken {\n source: test/source\n  ?? nope\n sink: test/sink\n}\n";
    let document = parse_document(source, &catalog());
    let diagnostic = document
        .diagnostics
        .first()
        .expect("invalid statement is diagnosed");

    assert_eq!(document.round_trip(), source);
    assert_eq!(diagnostic.code, "CND-FRM-013");
    assert_eq!(
        &source[diagnostic.span.start..diagnostic.span.end],
        "?? nope"
    );
    assert_eq!(diagnostic.span.line, 4);
    assert_eq!(diagnostic.span.column, 3);
    assert!(document.checked_form.is_none());
    assert!(document
        .tokens
        .iter()
        .any(|token| token.text == "test/sink"));
}

#[test]
fn missing_close_is_diagnosed_at_eof_without_losing_source() {
    let source = "form 0\nopen {\n source: test/source\n";
    let document = parse_document(source, &catalog());
    let diagnostic = document
        .diagnostics
        .first()
        .expect("missing close is diagnosed");

    assert_eq!(diagnostic.code, "CND-FRM-004");
    assert_eq!(diagnostic.span.start, source.len());
    assert_eq!(diagnostic.span.end, source.len());
    assert_eq!(document.round_trip(), source);
}

#[test]
fn lossless_document_preserves_distinct_source_and_checked_identities() {
    let baseline_source =
        "form 0\nidentity {\n source: test/source\n sink: test/sink\n source > sink\n}\n";
    let spelling_source = "# layout only\nform 0\n\nidentity {\n source: test/source\n sink: test/sink\n source > sink\n}\n";
    let baseline = parse_document(baseline_source, &catalog());
    let spelling = parse_document(spelling_source, &catalog());
    let baseline = baseline.checked().expect("baseline checks");
    let spelling = spelling.checked().expect("spelling checks");

    assert_ne!(baseline.source_document_id, spelling.source_document_id);
    assert_eq!(baseline.checked_form_id, spelling.checked_form_id);
    assert_eq!(baseline.expanded_form_id, spelling.expanded_form_id);
}

#[test]
fn lossless_document_enforces_source_and_token_bounds() {
    let oversized_source = " ".repeat(MAXIMUM_FORM_SOURCE_BYTES + 1);
    let source_document = parse_document(&oversized_source, &catalog());
    assert_eq!(source_document.diagnostics[0].code, "CND-FRM-014");
    assert!(matches!(
        parse(&oversized_source, &catalog()),
        Err(FormError::SourceLimitExceeded)
    ));

    let token_heavy_source = "x ".repeat(MAXIMUM_FORM_TOKENS + 1);
    assert!(token_heavy_source.len() < MAXIMUM_FORM_SOURCE_BYTES);
    let token_document = parse_document(&token_heavy_source, &catalog());
    assert_eq!(token_document.diagnostics[0].code, "CND-FRM-015");
    assert_eq!(token_document.round_trip(), token_heavy_source);
    assert!(token_document.checked_form.is_none());
}

#[test]
fn parses_catalog_supplied_kinds_and_ports() {
    let form = parse(
            "form 0\n\ndemo {\n source: test/source\n sink: test/sink\n source.count = 3\n source > sink\n}\n",
            &catalog(),
        )
        .expect("form parses");
    assert_eq!(form.gears[0].kind_id.as_str(), "test/sink");
    assert_eq!(form.gears[1].kind_id.as_str(), "test/source");
    assert_eq!(form.connections[0].source_port_id.as_str(), "out");
    assert_eq!(form.connections[0].sink_port_id.as_str(), "in");
}

#[test]
fn rejects_kinds_absent_from_catalog() {
    let error = parse("form 0\n\nbad {\n op: missing/kind\n}\n", &catalog())
        .expect_err("unknown kind fails");
    assert!(error.to_string().contains("missing/kind"));
}

#[test]
fn enforces_catalog_supplied_configuration_rules() {
    let error = parse(
        "form 0\n\ndemo {\n source: test/source\n source.count = 5\n}\n",
        &catalog(),
    )
    .expect_err("out-of-range catalog value fails");
    assert!(matches!(error, super::FormError::InvalidConfiguration(_)));
}

#[test]
fn checks_named_faces_against_exact_internal_endpoints() {
    let form = parse(
            "form 0\n\ncomposite {\n source: test/source\n sink: test/sink\n source > sink\n export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n}\n",
            &catalog(),
        )
        .expect("authored export parses");
    assert_eq!(form.exports.len(), 1);
    assert_eq!(form.exports[0].capability_id.as_str(), "run");
    assert_eq!(form.exports[0].kind_id.as_str(), "test/composite");
    assert_eq!(form.exports[0].input_faces.len(), 1);
    assert_eq!(form.exports[0].output_faces.len(), 1);
    assert_eq!(
        form.exports[0].input_faces[0]
            .external_port
            .value_kind
            .as_str(),
        "test/value"
    );

    let error = parse(
        "form 0\n\nbad {\n source: test/source\n sink: test/sink\n export run: test/composite {
  input in: test/value = source.out terminal independent
 }\n}\n",
        &catalog(),
    )
    .expect_err("an input face cannot map to an output endpoint");
    assert!(matches!(error, super::FormError::InvalidExport(_)));
}

#[test]
fn checked_export_is_the_only_source_of_a_parent_kind_boundary() {
    let source = "form 0\nchild {\n source: test/source\n sink: test/sink\n source > sink\n export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n}\n";
    let child = parse(source, &catalog()).expect("child checks");
    let capability_id = CapabilityId::from("run");
    let boundary = child
        .export_boundary(&capability_id)
        .expect("authored export derives a boundary");

    assert_eq!(boundary.kind_id.as_str(), "test/composite");
    assert_eq!(boundary.inputs.len(), 1);
    assert_eq!(boundary.inputs[0].port_id.as_str(), "in");
    assert_eq!(boundary.outputs.len(), 1);
    assert_eq!(boundary.outputs[0].port_id.as_str(), "out");
    assert_eq!(boundary.input_faces[0].internal_gear_id.as_str(), "sink");
    assert_eq!(boundary.output_faces[0].internal_gear_id.as_str(), "source");
    assert!(child
        .export_boundary(&CapabilityId::from("invented"))
        .is_err());

    let mut parent_catalog = catalog();
    let installed = parent_catalog
        .insert_export(&child, &capability_id)
        .expect("checked boundary installs");
    let parent = parse(
        "form 0\nparent {\n child: test/composite\n sink: test/sink\n child.out -> sink.in\n}\n",
        &parent_catalog,
    )
    .expect("ordinary parent cord checks");
    assert_eq!(
        parent.gears[0].kind_contract_revision,
        installed.kind_contract_revision
    );
    assert_eq!(parent.connections[0].source_port_id.as_str(), "out");

    let changed = parse(
            "form 0\nchild {\n source: test/source\n sink: test/sink\n source.count = 2\n source > sink\n export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n}\n",
            &catalog(),
        )
        .expect("semantic change checks");
    assert_eq!(
        boundary.kind_contract_revision,
        changed
            .export_boundary(&capability_id)
            .expect("changed export derives a boundary")
            .kind_contract_revision
    );
    assert_ne!(child.checked_form_id, changed.checked_form_id);
    assert_ne!(child.expanded_form_id, changed.expanded_form_id);
}

#[test]
fn checked_face_mutations_fail_closed() {
    let source = "form 0\nchild {\n source: test/source\n sink: test/sink\n export run: test/composite {\n  input in: test/value = sink.in terminal independent\n  output out: test/value = source.out terminal independent\n }\n}\n";
    let baseline = parse(source, &catalog()).expect("baseline checks");
    let capability = CapabilityId::from("run");

    let mut duplicate_name = baseline.clone();
    duplicate_name.exports[0].output_faces[0]
        .external_port
        .port_id = duplicate_name.exports[0].input_faces[0]
        .external_port
        .port_id
        .clone();
    assert!(duplicate_name.export_boundary(&capability).is_err());

    let mut direction = baseline.clone();
    direction.exports[0].input_faces[0].external_port.direction = PortDirection::Output;
    assert!(direction.export_boundary(&capability).is_err());

    let mut kind = baseline.clone();
    kind.exports[0].output_faces[0].external_port.value_kind = kind_id("test/other");
    assert!(kind.export_boundary(&capability).is_err());

    let mut endpoint = baseline.clone();
    endpoint.exports[0].input_faces[0].internal_port_id = port_id("missing");
    assert!(endpoint.export_boundary(&capability).is_err());

    let mut terminal = baseline;
    terminal.exports[0].output_faces[0].terminal = CompositeFaceTerminal::Coupled;
    assert!(terminal.export_boundary(&capability).is_err());
}

#[test]
fn multiple_typed_and_zero_sided_faces_check_as_ordinary_kinds() {
    let multi = parse(
            "form 0\nmulti {\n source-a: test/source\n sink-a: test/sink\n source-b: test/source-b\n sink-b: test/sink-b\n export run: test/multi {\n  input number-in: test/value = sink-a.in terminal independent\n  input bytes-in: test/bytes = sink-b.bytes terminal independent\n  output number-out: test/value = source-a.out terminal independent\n  output bytes-out: test/bytes = source-b.bytes terminal independent\n }\n}\n",
            &multi_value_catalog(),
        )
        .expect("two-input two-output export checks without an internal boundary cord");
    let boundary = multi
        .export_boundary(&CapabilityId::from("run"))
        .expect("multi boundary derives");
    assert_eq!(boundary.inputs.len(), 2);
    assert_eq!(boundary.outputs.len(), 2);
    assert_eq!(boundary.inputs[0].value_kind.as_str(), "test/value");
    assert_eq!(boundary.inputs[1].value_kind.as_str(), "test/bytes");

    let input_only = parse(
            "form 0\ningest {\n sink: test/sink\n export ingest: test/input-only {\n  input value: test/value = sink.in terminal independent\n }\n}\n",
            &catalog(),
        )
        .expect("input-only export checks");
    let input_boundary = input_only
        .export_boundary(&CapabilityId::from("ingest"))
        .expect("input-only boundary derives");
    assert_eq!(input_boundary.inputs.len(), 1);
    assert!(input_boundary.outputs.is_empty());

    let output_only = parse(
            "form 0\nproduce {\n source: test/source\n export produce: test/output-only {\n  output value: test/value = source.out terminal independent\n }\n}\n",
            &catalog(),
        )
        .expect("output-only export checks");
    let output_boundary = output_only
        .export_boundary(&CapabilityId::from("produce"))
        .expect("output-only boundary derives");
    assert!(output_boundary.inputs.is_empty());
    assert_eq!(output_boundary.outputs.len(), 1);

    let mut parent_catalog = multi_value_catalog();
    parent_catalog
        .insert_export(&multi, &CapabilityId::from("run"))
        .expect("multi export installs as an ordinary kind");
    let parent = parse(
            "form 0\nparent {\n source-a: test/source\n source-b: test/source-b\n child: test/multi\n sink-a: test/sink\n sink-b: test/sink-b\n source-a.out -> child.number-in\n source-b.bytes -> child.bytes-in\n child.number-out -> sink-a.in\n child.bytes-out -> sink-b.bytes\n}\n",
            &parent_catalog,
        )
        .expect("parent checks all exported faces through ordinary ports");
    assert_eq!(parent.connections.len(), 4);
    assert!(parent
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "child")
        .is_some_and(|gear| gear.inputs.len() == 2 && gear.outputs.len() == 2));
}

#[test]
fn duplicate_export_capabilities_are_rejected() {
    let error = parse(
            "form 0\nchild {\n source: test/source\n sink: test/sink\n source > sink\n export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n export run: test/other {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n}\n",
            &catalog(),
        )
        .expect_err("one capability cannot name two boundaries");
    assert!(matches!(error, FormError::InvalidExport(_)));
}

#[test]
fn inline_nested_form_uses_the_same_checked_boundary_as_a_standalone_form() {
    let standalone_source = "form 0\nchild {\n source: test/source\n sink: test/sink\n source > sink\n export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n}\n";
    let nested_source = "form 0\nparent {\n child: run {\n  source: test/source\n  sink: test/sink\n  source > sink\n  export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n }\n final: test/sink\n child.out -> final.in\n}\n";
    let standalone = parse(standalone_source, &catalog()).expect("standalone child checks");
    let parent = parse(nested_source, &catalog()).expect("inline nested form checks");
    let nested = &parent.nested_forms[0];
    let capability_id = CapabilityId::from("run");

    assert_eq!(nested.gear_id.as_str(), "child");
    assert_eq!(nested.export_capability_id, capability_id);
    assert_eq!(nested.form.checked_form_id, standalone.checked_form_id);
    assert_eq!(nested.form.expanded_form_id, standalone.expanded_form_id);
    assert_ne!(
        nested.form.source_document_id,
        standalone.source_document_id
    );
    assert_eq!(
        nested
            .form
            .export_boundary(&capability_id)
            .expect("nested boundary checks"),
        standalone
            .export_boundary(&capability_id)
            .expect("standalone boundary checks")
    );
    assert_eq!(parent.connections.len(), 1);
    assert_eq!(parent.connections[0].source_gear_id.as_str(), "child");
    assert_eq!(parent.connections[0].source_port_id.as_str(), "out");
    assert_eq!(parent.connections[0].sink_gear_id.as_str(), "final");
}

#[test]
fn parent_expanded_identity_binds_hidden_child_semantics_not_checked_boundary() {
    let baseline = parse(
            "form 0\nparent {\n child: run {\n  source: test/source\n  sink: test/sink\n  source.count = 1\n  source > sink\n  export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n }\n final: test/sink\n child.out -> final.in\n}\n",
            &catalog(),
        )
        .expect("baseline nested parent checks");
    let changed = parse(
            "form 0\nparent {\n child: run {\n  source: test/source\n  sink: test/sink\n  source.count = 2\n  source > sink\n  export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n }\n final: test/sink\n child.out -> final.in\n}\n",
            &catalog(),
        )
        .expect("changed nested parent checks");

    assert_ne!(
        baseline.nested_forms[0].form.checked_form_id,
        changed.nested_forms[0].form.checked_form_id
    );
    assert_eq!(
        baseline.gears[0].kind_contract_revision,
        changed.gears[0].kind_contract_revision
    );
    assert_eq!(baseline.checked_form_id, changed.checked_form_id);
    assert_ne!(baseline.expanded_form_id, changed.expanded_form_id);
    baseline
        .validate_identities()
        .expect("baseline identities validate");
    changed
        .validate_identities()
        .expect("changed identities validate");
}

#[test]
fn nested_expansion_paths_are_canonical_and_substitution_fails_closed() {
    let baseline = parse(
            "form 0\nparent {\n left: run {\n  source: test/source\n  sink: test/sink\n  source.count = 1\n  source > sink\n  export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n }\n right: run {\n  source: test/source\n  sink: test/sink\n  source.count = 2\n  source > sink\n  export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n }\n left-sink: test/sink\n right-sink: test/sink\n left.out -> left-sink.in\n right.out -> right-sink.in\n}\n",
            &catalog(),
        )
        .expect("two nested paths check");
    let source_reordered = parse(
            "form 0\nparent {\n right: run {\n  source: test/source\n  sink: test/sink\n  source.count = 2\n  source > sink\n  export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n }\n left: run {\n  source: test/source\n  sink: test/sink\n  source.count = 1\n  source > sink\n  export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n }\n left-sink: test/sink\n right-sink: test/sink\n left.out -> left-sink.in\n right.out -> right-sink.in\n}\n",
            &catalog(),
        )
        .expect("source-reordered nested paths check");
    let implementations_swapped = parse(
            "form 0\nparent {\n left: run {\n  source: test/source\n  sink: test/sink\n  source.count = 2\n  source > sink\n  export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n }\n right: run {\n  source: test/source\n  sink: test/sink\n  source.count = 1\n  source > sink\n  export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n }\n left-sink: test/sink\n right-sink: test/sink\n left.out -> left-sink.in\n right.out -> right-sink.in\n}\n",
            &catalog(),
        )
        .expect("swapped nested implementations check");

    assert_eq!(baseline.checked_form_id, source_reordered.checked_form_id);
    assert_eq!(baseline.expanded_form_id, source_reordered.expanded_form_id);
    assert_eq!(
        baseline.checked_form_id,
        implementations_swapped.checked_form_id
    );
    assert_ne!(
        baseline.expanded_form_id,
        implementations_swapped.expanded_form_id
    );
    assert_eq!(baseline.nested_forms[0].gear_id.as_str(), "left");
    assert_eq!(baseline.nested_forms[1].gear_id.as_str(), "right");

    let mut omitted = baseline.clone();
    omitted.nested_forms.remove(0);
    assert!(matches!(
        omitted.validate_identities(),
        Err(FormError::InvalidIdentity(_))
    ));

    let mut duplicated = baseline.clone();
    duplicated
        .nested_forms
        .push(duplicated.nested_forms[0].clone());
    assert!(matches!(
        duplicated.validate_identities(),
        Err(FormError::InvalidIdentity(_))
    ));

    let mut reordered = baseline.clone();
    reordered.nested_forms.swap(0, 1);
    assert!(matches!(
        reordered.validate_identities(),
        Err(FormError::InvalidIdentity(_))
    ));

    let mut substituted = baseline;
    substituted.nested_forms[0].form = implementations_swapped.nested_forms[0].form.clone();
    assert!(matches!(
        substituted.validate_identities(),
        Err(FormError::InvalidIdentity(_))
    ));
}

#[test]
fn nested_errors_keep_the_outer_document_and_exact_inner_span() {
    let source = "form 0\nparent {\n child: run {\n  source: test/source\n  ?? inner error\n  sink: test/sink\n  source > sink\n  export run: test/composite {
  input in: test/value = sink.in terminal independent
  output out: test/value = source.out terminal independent
 }\n }\n}\n";
    let document = parse_document(source, &catalog());
    let diagnostic = &document.diagnostics[0];

    assert_eq!(document.round_trip(), source);
    assert_eq!(diagnostic.code, "CND-FRM-013");
    assert_eq!(diagnostic.span.line, 5);
    assert_eq!(
        &source[diagnostic.span.start..diagnostic.span.end],
        "?? inner error"
    );
    assert!(document.checked_form.is_none());
    assert!(document
        .tokens
        .iter()
        .any(|token| token.text == "test/sink"));
}

#[test]
fn inline_nesting_has_a_hard_depth_ceiling() {
    let mut source = String::from("form 0\nroot {\n");
    for depth in 0..=MAXIMUM_FORM_NESTING_DEPTH {
        source.push_str(&format!("n{depth}: run {{\n"));
    }
    let document = parse_document(&source, &catalog());

    assert_eq!(document.diagnostics[0].code, "CND-FRM-016");
    assert!(document.checked_form.is_none());
}
