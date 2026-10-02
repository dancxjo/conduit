use conduit_core::{
    kind_id, port_id, CapabilityLimits, ExternalEffectBehavior, Kind, KindIdentity,
    KindSemanticLaw, PortDescriptor, PortDirection, PortTemporal, ReplayBehavior,
    SemanticDependence, StructuredInfoType, SuspensionBehavior, TemporalStateBehavior,
    VariabilityBehavior,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, KindProjection,
    KindSignature, ProfileCatalog, StartupCatalog, PURE_EXPRESSION_REVISION,
};

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

fn pure_kind() -> Kind {
    Kind {
        kind_id: kind_id("math/negate"),
        kind_contract_revision: KindIdentity::from("math/negate@1"),
        startup_parameters: vec![],
        shorthand: Some((port_id("value"), port_id("result"))),
        inputs: vec![port("value", PortDirection::Input)],
        outputs: vec![port("result", PortDirection::Output)],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
            KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
            KindSemanticLaw::TimeDependence(SemanticDependence::None),
            KindSemanticLaw::RandomDependence(SemanticDependence::None),
            KindSemanticLaw::ResourceDependence(SemanticDependence::None),
            KindSemanticLaw::Suspension(SuspensionBehavior::Never),
            KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
            KindSemanticLaw::Replay(ReplayBehavior::Exact),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 8,
        },
    }
}

fn pure_binary_kind() -> Kind {
    Kind {
        kind_id: kind_id("math/add"),
        kind_contract_revision: KindIdentity::from("math/add@1"),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![
            port("left", PortDirection::Input),
            port("right", PortDirection::Input),
        ],
        outputs: vec![port("result", PortDirection::Output)],
        configuration: vec![],
        semantic_laws: pure_kind().semantic_laws,
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 16,
        },
    }
}

fn catalogs(kind: Kind) -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    for kind in ["test/scalar-source", "test/scalar-sink"] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
    }
    let mut profile = ProfileCatalog::new();
    profile.insert_kind(kind).unwrap();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/scalar-source"),
            kind_contract_revision: KindIdentity::from("test/scalar-source@1"),
            inputs: vec![],
            outputs: vec![port("out", PortDirection::Output)],
            configuration: vec![],
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/scalar-sink"),
            kind_contract_revision: KindIdentity::from("test/scalar-sink@1"),
            inputs: vec![port("in", PortDirection::Input)],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    (startup, profile)
}

#[test]
fn nested_pure_semantic_calls_lower_to_ordered_called_kind_gears_not_host_calls() {
    let (startup, profile) = catalogs(pure_kind());
    let source = "plot calculate {\n source: test/scalar-source\n sink: test/scalar-sink\n source >> (math/negate(math/negate(.))) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "calculate", &profile).unwrap();
    assert!(expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == "math/negate"));
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "math/negate")
            .count(),
        2
    );
    assert!(expanded
        .gears
        .iter()
        .all(|gear| gear.kind_contract_revision.as_str() != PURE_EXPRESSION_REVISION));
    assert_eq!(expanded.connections.len(), 3);
    expanded.validate_expansion().unwrap();
}

#[test]
fn repeated_input_arguments_lower_to_one_multi_input_semantic_gear() {
    let (startup, profile) = catalogs(pure_binary_kind());
    let source = "plot calculate {\n source: test/scalar-source\n sink: test/scalar-sink\n source >> (math/add(., .)) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "calculate", &profile).unwrap();
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "math/add")
            .count(),
        1
    );
    assert!(expanded
        .gears
        .iter()
        .all(|gear| gear.kind_contract_revision.as_str() != PURE_EXPRESSION_REVISION));
    assert_eq!(expanded.connections.len(), 3);
    expanded.validate_expansion().unwrap();
}

#[test]
fn literal_call_argument_lowers_to_an_admitted_expression_gear() {
    let (startup, profile) = catalogs(pure_binary_kind());
    let source = "plot calculate {\n one = 1\n source: test/scalar-source\n sink: test/scalar-sink\n source >> (math/add(., one)) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "calculate", &profile).unwrap();
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "math/add")
            .count(),
        1
    );
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_contract_revision.as_str() == PURE_EXPRESSION_REVISION)
            .count(),
        1
    );
    assert_eq!(expanded.connections.len(), 4);
    expanded.validate_expansion().unwrap();
}

#[test]
fn semantic_call_beneath_operator_lowers_to_call_then_pure_expression() {
    let (startup, profile) = catalogs(pure_kind());
    let source = "plot calculate {\n source: test/scalar-source\n sink: test/scalar-sink\n source >> (math/negate(.) + 1) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "calculate", &profile).unwrap();
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "math/negate")
            .count(),
        1
    );
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_contract_revision.as_str() == PURE_EXPRESSION_REVISION)
            .count(),
        1
    );
    assert_eq!(expanded.connections.len(), 3);
    expanded.validate_expansion().unwrap();
}

#[test]
fn semantic_call_beneath_structure_lowers_to_call_then_pure_expression() {
    let (mut startup, mut profile) = catalogs(pure_kind());
    let pair = StructuredInfoType::collection(
        StructuredInfoType::leaf(kind_id(conduit_core::SCALAR_INFO_ID)).unwrap(),
        Some(2),
    )
    .unwrap();
    let pair_kind = pair.profile().unwrap().value_kind().clone();
    startup.insert_structured_type("ScalarPair", pair).unwrap();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/pair-sink"),
            kind_contract_revision: KindIdentity::from("test/pair-sink@1"),
            inputs: vec![PortDescriptor {
                value_kind: pair_kind,
                ..port("in", PortDirection::Input)
            }],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    startup
        .insert(KindSignature {
            kind: "test/pair-sink".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let source = "plot calculate {\n source: test/scalar-source\n sink: test/pair-sink\n source >> ([math/negate(.), 1]) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "calculate", &profile).unwrap();
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_contract_revision.as_str() == PURE_EXPRESSION_REVISION)
            .count(),
        1
    );
    assert_eq!(expanded.connections.len(), 3);
    expanded.validate_expansion().unwrap();
}

#[test]
fn outer_expression_cannot_implicitly_synchronize_two_call_results() {
    let (startup, profile) = catalogs(pure_kind());
    let source = "plot calculate {\n source: test/scalar-source\n sink: test/scalar-sink\n source >> (math/negate(.) + math/negate(.)) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let refusal = expand_canonical_plot(&checked, "calculate", &profile).unwrap_err();
    assert!(refusal.message.contains("multiple semantic call results"));
    for family in ["flow/zip", "state/combine-latest", "flow/join/by-key"] {
        assert!(refusal.message.contains(family));
    }
}

#[test]
fn outer_expression_cannot_reuse_independent_input_beside_call_result() {
    let (startup, profile) = catalogs(pure_kind());
    let source = "plot calculate {\n source: test/scalar-source\n sink: test/scalar-sink\n source >> (math/negate(.) + .) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let refusal = expand_canonical_plot(&checked, "calculate", &profile).unwrap_err();
    assert!(refusal
        .message
        .contains("depend only on that call result and constants"));
    assert!(refusal.message.contains("current/sample"));
    assert!(refusal.message.contains("two independent runtime values"));
}
