use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

const SOURCE: &str = include_str!("../../../forms/experiencer/main.conduit");

#[test]
fn experiencer_is_one_portable_typed_convergence_without_effect_authority() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_presentation::install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_experience_catalogs(&mut startup, &mut profile).unwrap();

    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored = expand_canonical_form_for_authoring(&checked, "experiencer", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        "experience/relate-current"
    );
    assert_eq!(authored.expanded.gears[0].inputs.len(), 7);
    assert_eq!(authored.expanded.gears[0].outputs.len(), 1);

    for forbidden in [
        "camera",
        "microphone",
        "filesystem",
        "socket",
        "host/",
        "execute-action",
        "fulfill",
    ] {
        assert!(
            !SOURCE.contains(forbidden),
            "portable source leaked {forbidden}"
        );
    }

    let definition = profile
        .get(&conduit_core::kind_id(
            conduit_semantic_catalog::EXPERIENCE_RELATE_KIND,
        ))
        .unwrap();
    assert!(definition
        .inputs
        .iter()
        .all(|port| port.temporal == conduit_core::PortTemporal::Flow { closes: false }));
    assert_eq!(
        definition.outputs[0].temporal,
        conduit_core::PortTemporal::Current
    );
}

#[test]
fn epistemic_source_types_remain_nominally_distinct_and_bounded() {
    let types = [
        conduit_semantic_catalog::experience_human_input_type(),
        conduit_semantic_catalog::experience_body_input_type(),
        conduit_semantic_catalog::experience_memory_input_type(),
        conduit_semantic_catalog::experience_inference_input_type(),
        conduit_semantic_catalog::experience_hypothesis_input_type(),
    ];
    let kinds = types
        .iter()
        .map(|value| value.profile().unwrap().value_kind().clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(kinds.len(), types.len());

    let current = conduit_semantic_catalog::current_experience_type();
    let conduit_core::StructuredInfoTypeShape::Record { fields, .. } = current.shape() else {
        panic!("current experience must remain a nominal record")
    };
    let identities = fields
        .iter()
        .find(|field| field.name() == "item_identities")
        .unwrap();
    let conduit_core::StructuredInfoTypeShape::Collection { length, .. } =
        identities.value_type().shape()
    else {
        panic!("item identities must remain bounded")
    };
    assert_eq!(length, 32);
    assert!(fields
        .iter()
        .any(|field| field.name() == "provenance_revision"));
}
