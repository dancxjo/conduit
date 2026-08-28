use conduit_core::{
    port_id, KindContractRevision, KindId, PortDescriptor, PortDirection, PortTemporal,
    StructuredFieldType, StructuredInfoType,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
};

fn event_type(extra_field: bool) -> StructuredInfoType {
    let count = StructuredInfoType::leaf(KindId::from("value/count@1")).unwrap();
    let mut fields = vec![StructuredFieldType::new("pitch", count.clone()).unwrap()];
    if extra_field {
        fields.push(StructuredFieldType::new("velocity", count).unwrap());
    }
    StructuredInfoType::record(KindId::from("music/note@1"), fields).unwrap()
}

#[test]
fn runtime_ports_use_semantic_profile_identity_instead_of_alias_spelling() {
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_structured_type("MusicEvent", event_type(false))
        .unwrap();
    catalog
        .insert_structured_type("RenamedEvent", event_type(false))
        .unwrap();
    let source = "form source (\n event: MusicEvent >\n) {\n}\n\nform sink (\n > event: RenamedEvent\n) {\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &catalog).unwrap();
    let source_face = checked.forms[1].checked_face();
    let sink_face = checked.forms[0].checked_face();
    let source_kind = &source_face.outputs()[0].value_kind;
    let sink_kind = &sink_face.inputs()[0].value_kind;

    assert_eq!(source_kind, sink_kind);
    assert!(source_kind.as_str().starts_with("structured-info/profile-"));
    assert!(!source_kind.as_str().contains("MusicEvent"));
}

#[test]
fn changing_shape_changes_checked_identity_and_port_compatibility() {
    let source = "form source (\n event: MusicEvent >\n) {\n}\n";
    let parsed = parse_syntax_document(source);
    let mut first_catalog = StartupCatalog::new();
    first_catalog
        .insert_structured_type("MusicEvent", event_type(false))
        .unwrap();
    let mut second_catalog = StartupCatalog::new();
    second_catalog
        .insert_structured_type("MusicEvent", event_type(true))
        .unwrap();
    let first = check_syntax_document(&parsed, &first_catalog).unwrap();
    let second = check_syntax_document(&parsed, &second_catalog).unwrap();

    assert_ne!(
        first.forms[0].checked_form_id,
        second.forms[0].checked_form_id
    );
    assert_ne!(
        first.forms[0].checked_face().outputs()[0].value_kind,
        second.forms[0].checked_face().outputs()[0].value_kind
    );
}

#[test]
fn ordinary_unregistered_port_kinds_keep_the_existing_exact_vocabulary() {
    let source = "form source (\n text: Text >\n exact: domain/custom@2 >\n) {\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let face = checked.forms[0].checked_face();
    let outputs = face.outputs();
    assert_eq!(outputs[0].value_kind.as_str(), "domain/custom@2");
    assert_eq!(outputs[1].value_kind.as_str(), "value/text@1");
}

#[test]
fn canonical_expansion_checks_the_resolved_profile_not_the_alias() {
    let value_type = event_type(false);
    let profile_kind = value_type.profile().unwrap().value_kind().clone();
    let mut startup = StartupCatalog::new();
    startup
        .insert_structured_type("MusicEvent", value_type)
        .unwrap();
    startup
        .insert(KindSignature {
            kind: "music/source".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let source = "form source (\n event: MusicEvent >\n) {\n primitive: music/source\n primitive > event\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();

    let definition = |value_kind| KindDefinition {
        kind_id: KindId::from("music/source"),
        kind_contract_revision: KindContractRevision::from("music/source@1"),
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("event"),
            value_kind,
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        configuration: vec![],
    };
    let mut matching = ProfileCatalog::new();
    matching.insert(definition(profile_kind)).unwrap();
    expand_canonical_form_for_authoring(&checked, "source", &matching).unwrap();

    let mut mismatched = ProfileCatalog::new();
    mismatched
        .insert(definition(
            event_type(true).profile().unwrap().value_kind().clone(),
        ))
        .unwrap();
    let error = expand_canonical_form_for_authoring(&checked, "source", &mismatched).unwrap_err();
    assert_eq!(error.code, "CND-FRM-045");
}
