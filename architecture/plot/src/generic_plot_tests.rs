use crate::{check_syntax_document, parse_syntax_document, CheckedSyntaxDocument, StartupCatalog};

fn check(source: &str) -> CheckedSyntaxDocument {
    let parsed = parse_syntax_document(source);
    check_syntax_document(&parsed, &StartupCatalog::new()).expect("generic source checks")
}

#[test]
fn named_type_parameters_specialize_to_exact_ordinary_fores() {
    let checked = check(
        "plot identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         plot text-identity (\n >> value: Text\n result: Text >>\n) {\n value >> identity(item = Text) >> result\n}\n\
         plot bytes-identity (\n >> value: Bytes\n result: Bytes >>\n) {\n value >> identity(item = Bytes) >> result\n}\n",
    );

    assert!(checked.plots.iter().all(|plot| plot.name != "identity"));
    let text = checked
        .plots
        .iter()
        .find(|plot| plot.name == "identity[item=value/text]")
        .expect("Text specialization is retained as an exact ordinary Plot");
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
        .plots
        .iter()
        .find(|plot| plot.name == "identity[item=value/bytes]")
        .expect("Bytes specialization is retained as an exact ordinary Plot");
    assert_eq!(
        bytes.runtime_front.inputs()[0].value_kind,
        conduit_core::kind_id("value/bytes")
    );
    assert_ne!(text.checked_plot_id, bytes.checked_plot_id);

    for wrapper in ["text-identity", "bytes-identity"] {
        let plot = checked
            .plots
            .iter()
            .find(|plot| plot.name == wrapper)
            .unwrap();
        assert!(plot.gears[0].startup_bindings.is_empty());
        assert!(plot.gears[0].kind.starts_with("identity[item=value/"));
    }
}

#[test]
fn explicit_generic_application_cannot_disagree_with_connected_ports() {
    let parsed = parse_syntax_document(
        "plot identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         plot bad (\n >> value: Text\n result: Text >>\n) {\n value >> identity(item = Bytes) >> result\n}\n",
    );
    let error = check_syntax_document(&parsed, &StartupCatalog::new())
        .expect_err("Text cannot satisfy a Bytes specialization");
    assert_eq!(error.code, "CND-FRM-057");
    assert!(error.message.contains("connected ports require"));
}

#[test]
fn unary_generic_application_infers_one_unambiguous_port_type() {
    let checked = check(
        "plot identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         plot main (\n >> value: Text\n result: Text >>\n) {\n value >> identity() >> result\n}\n",
    );
    let main = checked
        .plots
        .iter()
        .find(|plot| plot.name == "main")
        .unwrap();
    assert_eq!(main.gears[0].kind, "identity[item=value/text]");
    assert!(main.gears[0].startup_bindings.is_empty());
}

#[test]
fn generic_inference_refuses_conflicting_port_evidence() {
    let parsed = parse_syntax_document(
        "plot identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         plot bad (\n >> value: Text\n result: Bytes >>\n) {\n value >> identity() >> result\n}\n",
    );
    let error = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-057");
    assert!(error.message.contains("conflicting types"));
}

#[test]
fn generic_specialization_preserves_temporal_modalities_and_data_references() {
    let checked = check(
        "plot latest (\n item: type\n >> values: item...\n >> source: $item\n current: $item >>\n snapshot: &item >>\n) {\n source >> current\n}\n\
         plot main {\n selected: latest(item = Text)\n}\n",
    );
    let latest = checked
        .plots
        .iter()
        .find(|plot| plot.name == "latest[item=value/text]")
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
    let template = "plot library/identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n";
    let direct = check(&format!(
        "{template}plot main (\n >> value: Text\n result: Text >>\n) {{\n value >> library/identity(item = Text) >> result\n}}\n"
    ));
    let aliased = check(&format!(
        "with library/identity as copy\n{template}plot main (\n >> value: Text\n result: Text >>\n) {{\n value >> copy(item = Text) >> result\n}}\n"
    ));
    let direct_main = direct
        .plots
        .iter()
        .find(|plot| plot.name == "main")
        .unwrap();
    let aliased_main = aliased
        .plots
        .iter()
        .find(|plot| plot.name == "main")
        .unwrap();
    assert_eq!(direct_main.checked_plot_id, aliased_main.checked_plot_id);
    assert_eq!(
        direct_main.gears[0].kind,
        "library/identity[item=value/text]"
    );
    assert_eq!(direct_main.gears[0].kind, aliased_main.gears[0].kind);
}

#[test]
fn type_arguments_must_resolve_to_exact_checked_types() {
    let parsed = parse_syntax_document(
        "plot identity (\n item: type\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         plot bad {\n value: identity(item = Mystery)\n}\n",
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
        "plot identity (\n item: type\n >> value: item\n result: item >>\n) {\n pass: test/pass\n value >> pass.input\n pass.output >> result\n}\n\
         plot main (\n >> value: Text\n result: Text >>\n) {\n copy: identity(item = Text)\n value >> copy.value\n copy.result >> result\n}\n",
    );
    let checked = check_syntax_document(&parsed, &profiles.startup_catalog().unwrap()).unwrap();
    let authoring =
        crate::expand_canonical_plot_for_authoring(&checked, "main", &profiles).unwrap();
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
        "plot repeat (\n item: type\n copies: Count = 1\n >> value: item\n result: item >>\n) {\n value >> result\n}\n\
         plot main {\n repeated: repeat(item = Text, copies = 2)\n}\n",
    );
    let main = checked
        .plots
        .iter()
        .find(|plot| plot.name == "main")
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

#[test]
fn type_parameter_cannot_masquerade_as_a_runtime_front_name() {
    let parsed = parse_syntax_document("plot bad (\n item: type\n >> item: Text\n) {\n}\n");
    let error = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-050");
    assert!(error.message.contains("item"));
}

#[test]
fn specialization_propagates_finite_bounds_into_startup_and_retained_state() {
    let parsed = parse_syntax_document(
        "plot cache (\n item: type\n initial: item\n) {\n cell: keep item for this play\n}\n\
         plot main {\n cache: cache(item = Text, initial = \"ready\")\n}\n",
    );
    let mut catalog = StartupCatalog::new();
    catalog
        .insert(crate::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let checked = check_syntax_document(&parsed, &catalog).unwrap();
    let cache = checked
        .plots
        .iter()
        .find(|plot| plot.name == "cache[item=value/text]")
        .unwrap();
    assert_eq!(cache.startup_parameters[0].maximum_bytes, Some(256));
    let retained = cache.gears[0].retained.as_ref().unwrap();
    assert_eq!(retained.value_kind, conduit_core::kind_id("value/text"));
    assert_eq!(retained.maximum_bytes, Some(256));
}

#[test]
fn named_parameters_specialize_independently_to_structured_and_scalar_types() {
    let parsed = parse_syntax_document(
        "plot pair (\n left: type\n right: type\n >> a: left\n >> b: right\n first: left >>\n second: right >>\n) {\n a >> first\n b >> second\n}\n\
         plot main {\n pair: pair(left = Pair, right = Count)\n}\n",
    );
    let pair_type = conduit_core::StructuredInfoType::collection(
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/text")).unwrap(),
        Some(2),
    )
    .unwrap();
    let pair_kind = pair_type.profile().unwrap().value_kind().clone();
    let mut catalog = StartupCatalog::new();
    catalog.insert_structured_type("Pair", pair_type).unwrap();
    let checked = check_syntax_document(&parsed, &catalog).unwrap();
    let pair = checked
        .plots
        .iter()
        .find(|plot| {
            plot.name
                .contains(&format!("left={},right=value/count", pair_kind.as_str()))
        })
        .unwrap();
    assert_eq!(pair.runtime_front.inputs()[0].value_kind, pair_kind);
    assert_eq!(
        pair.runtime_front.inputs()[1].value_kind,
        conduit_core::kind_id("value/count")
    );
}

#[test]
fn exact_source_plot_behavior_parameter_specializes_to_an_ordinary_gear() {
    let checked = check(
        "plot text/upper (\n >> value: Text\n mapped: Text >>\n) {\n value >> mapped\n}\n\nplot apply (\n transform: kind (\n  >> value: Text\n  mapped: Text >>\n )\n >> value: Text\n mapped: Text >>\n) {\n value >> transform() >> mapped\n}\n\nplot main {\n applied: apply(transform = text/upper)\n}\n",
    );
    assert!(checked.plots.iter().all(|plot| plot.name != "apply"));
    let specialized = checked
        .plots
        .iter()
        .find(|plot| plot.name == "apply[transform=text/upper]")
        .unwrap();
    assert!(specialized.startup_parameters.is_empty());
    assert_eq!(specialized.gears[0].kind, "text/upper");
    assert!(specialized.gears[0].startup_bindings.is_empty());
}

#[test]
fn behavior_parameter_refuses_an_incompatible_named_fore() {
    let parsed = parse_syntax_document(
        "plot wrong (\n >> other: Text\n mapped: Text >>\n) {\n other >> mapped\n}\n\nplot apply (\n transform: kind (\n  >> value: Text\n  mapped: Text >>\n )\n >> value: Text\n mapped: Text >>\n) {\n value >> transform() >> mapped\n}\n\nplot main {\n applied: apply(transform = wrong)\n}\n",
    );
    let error = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap_err();
    assert_eq!(error.code, "CND-FRM-057");
    assert!(error.message.contains("incompatible exact Fore"));
}

#[test]
fn multi_port_behavior_parameter_preserves_every_named_fore_role() {
    let checked = check(
        "plot pair/swap (\n >> left: Text\n >> right: Text\n first: Text >>\n second: Text >>\n) {\n left >> second\n right >> first\n}\n\nplot apply-pair (\n operation: kind (\n  >> left: Text\n  >> right: Text\n  first: Text >>\n  second: Text >>\n )\n >> left: Text\n >> right: Text\n first: Text >>\n second: Text >>\n) {\n selected: operation()\n left >> selected.left\n right >> selected.right\n selected.first >> first\n selected.second >> second\n}\n\nplot main {\n pair: apply-pair(operation = pair/swap)\n}\n",
    );
    let specialized = checked
        .plots
        .iter()
        .find(|plot| plot.name == "apply-pair[operation=pair/swap]")
        .unwrap();
    assert_eq!(specialized.gears[0].kind, "pair/swap");
    assert_eq!(specialized.runtime_front.inputs().len(), 2);
    assert_eq!(specialized.runtime_front.outputs().len(), 2);
}

#[test]
fn installed_kind_and_source_plot_arguments_share_the_same_fore_check() {
    let port = |name: &str, direction| conduit_core::PortDescriptor {
        port_id: conduit_core::port_id(name),
        value_kind: conduit_core::kind_id("value/text"),
        direction,
        temporal: conduit_core::PortTemporal::Value,
        abnormal_kind: None,
    };
    let mut catalog = StartupCatalog::new();
    catalog
        .insert(crate::KindSignature {
            kind: "text/installed-upper".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    catalog
        .insert_fore(
            "text/installed-upper",
            conduit_core::CheckedFront::new(
                vec![],
                vec![port("value", conduit_core::PortDirection::Input)],
                vec![port("mapped", conduit_core::PortDirection::Output)],
                None,
            ),
        )
        .unwrap();
    let parsed = parse_syntax_document(
        "plot apply (\n transform: kind (\n  >> value: Text\n  mapped: Text >>\n )\n >> value: Text\n mapped: Text >>\n) {\n value >> transform() >> mapped\n}\n\nplot main {\n applied: apply(transform = text/installed-upper)\n}\n",
    );
    let checked = check_syntax_document(&parsed, &catalog).unwrap();
    let specialized = checked
        .plots
        .iter()
        .find(|plot| plot.name == "apply[transform=text/installed-upper]")
        .unwrap();
    assert_eq!(specialized.gears[0].kind, "text/installed-upper");
}

#[test]
fn supplied_behavior_laws_survive_specialization_into_the_ordinary_graph() {
    use conduit_core::{
        AbnormalTerminalTransduction, CancellationTransduction, ExternalEffectBehavior,
        KindSemanticLaw, NormalCloseTransduction, SemanticDependence, SuspensionBehavior,
        TemporalStateBehavior, TerminalTransductionProfile,
    };

    let port = |name: &str, direction| conduit_core::PortDescriptor {
        port_id: conduit_core::port_id(name),
        value_kind: conduit_core::kind_id("value/text"),
        direction,
        temporal: conduit_core::PortTemporal::Value,
        abnormal_kind: None,
    };
    let mut laws = crate::pure_expression_semantic_laws();
    laws[0] = KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable);
    laws[1] = KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained);
    laws[4] = KindSemanticLaw::ResourceDependence(SemanticDependence::Ambient);
    laws[5] = KindSemanticLaw::Suspension(SuspensionBehavior::MaySuspend);
    laws.push(KindSemanticLaw::TerminalTransduction(
        TerminalTransductionProfile {
            input_port_id: conduit_core::port_id("value"),
            output_port_id: conduit_core::port_id("mapped"),
            normal_close: NormalCloseTransduction::NotAccepted,
            abnormal: AbnormalTerminalTransduction::NotAccepted,
            cancellation: CancellationTransduction::NotCancellable,
        },
    ));
    let kind = conduit_core::Kind {
        kind_id: conduit_core::kind_id("text/effectful"),
        kind_contract_revision: conduit_core::KindIdentity::from("text/effectful@1"),
        startup_parameters: vec![],
        shorthand: Some((
            conduit_core::port_id("value"),
            conduit_core::port_id("mapped"),
        )),
        inputs: vec![port("value", conduit_core::PortDirection::Input)],
        outputs: vec![port("mapped", conduit_core::PortDirection::Output)],
        configuration: vec![],
        semantic_laws: laws.clone(),
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 256,
        },
    };
    let mut profile = crate::ProfileCatalog::new();
    profile.insert_kind(kind).unwrap();
    let parsed = parse_syntax_document(
        "plot apply (\n transform: kind (\n  value: Text >> mapped: Text\n )\n value: Text >> mapped: Text\n) {\n value >> transform() >> mapped\n}\n\nplot main {\n applied: apply(transform = text/effectful)\n}\n",
    );
    let checked = check_syntax_document(&parsed, &profile.startup_catalog().unwrap()).unwrap();
    let specialized = checked
        .plots
        .iter()
        .find(|plot| plot.name == "apply[transform=text/effectful]")
        .unwrap();
    let expanded =
        crate::expand_canonical_plot_for_authoring(&checked, &specialized.name, &profile).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    assert_eq!(expanded.expanded.gears[0].semantic_contract.laws, laws);
    assert_eq!(expanded.expanded.gears[0].terminal_transductions.len(), 1);
}

#[test]
fn same_fore_effectful_behavior_refuses_when_the_use_requires_purity() {
    let port = |name: &str, direction| conduit_core::PortDescriptor {
        port_id: conduit_core::port_id(name),
        value_kind: conduit_core::kind_id("value/scalar"),
        direction,
        temporal: conduit_core::PortTemporal::Value,
        abnormal_kind: None,
    };
    let mut laws = crate::pure_expression_semantic_laws();
    laws[0] = conduit_core::KindSemanticLaw::ExternalEffects(
        conduit_core::ExternalEffectBehavior::Observable,
    );
    let mut profile = crate::ProfileCatalog::new();
    profile
        .insert_kind(conduit_core::Kind {
            kind_id: conduit_core::kind_id("test/effect"),
            kind_contract_revision: conduit_core::KindIdentity::from("test/effect@1"),
            startup_parameters: vec![],
            shorthand: Some((
                conduit_core::port_id("value"),
                conduit_core::port_id("result"),
            )),
            inputs: vec![port("value", conduit_core::PortDirection::Input)],
            outputs: vec![port("result", conduit_core::PortDirection::Output)],
            configuration: vec![],
            semantic_laws: laws,
            limits: conduit_core::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 8,
            },
        })
        .unwrap();
    let parsed = parse_syntax_document(
        "plot apply (\n transform: kind (\n  value: Scalar >> result: Scalar\n )\n value: Scalar >> result: Scalar\n) = (transform(.))\n\nplot main {\n applied: apply(transform = test/effect)\n}\n",
    );
    let checked = check_syntax_document(&parsed, &profile.startup_catalog().unwrap()).unwrap();
    let refusal = crate::expand_canonical_plot_for_authoring(
        &checked,
        "apply[transform=test/effect]",
        &profile,
    )
    .unwrap_err();
    assert_eq!(refusal.code, "CND-FRM-046");
    assert!(
        refusal
            .message
            .contains("ineligible for pure expression use"),
        "{}",
        refusal.message
    );
}
