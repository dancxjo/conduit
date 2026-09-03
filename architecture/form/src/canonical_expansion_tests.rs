use crate::prelude::*;

use crate::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, ConfigurationValue, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};
use conduit_core::{kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection};

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id("test/value"),
        direction,
        temporal: conduit_core::PortTemporal::Value,
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    for signature in [
        KindSignature {
            kind: "test/source".into(),
            startup_parameters: vec![],
        },
        KindSignature {
            kind: "test/pass".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "count".into(),
                value_type: "Count".into(),
                default: Some("1".into()),
            }],
        },
        KindSignature {
            kind: "test/sink".into(),
            startup_parameters: vec![],
        },
        KindSignature {
            kind: "test/use-pool".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "members".into(),
                value_type: "Pool".into(),
                default: None,
            }],
        },
    ] {
        startup.insert(signature).unwrap();
    }
    let mut profile = ProfileCatalog::new();
    for definition in [
        KindDefinition {
            kind_id: kind_id("test/source"),
            kind_contract_revision: KindContractRevision::from("test/source@1"),
            inputs: vec![],
            outputs: vec![port("out", PortDirection::Output)],
            configuration: vec![],
        },
        KindDefinition {
            kind_id: kind_id("test/use-pool"),
            kind_contract_revision: KindContractRevision::from("test/use-pool@1"),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![],
        },
        KindDefinition {
            kind_id: kind_id("test/pass"),
            kind_contract_revision: KindContractRevision::from("test/pass@1"),
            inputs: vec![port("in", PortDirection::Input)],
            outputs: vec![port("out", PortDirection::Output)],
            configuration: vec![ConfigurationField {
                key: "count".into(),
                default_value: ConfigurationValue::U64(1),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: 8,
                },
            }],
        },
        KindDefinition {
            kind_id: kind_id("test/sink"),
            kind_contract_revision: KindContractRevision::from("test/sink@1"),
            inputs: vec![port("in", PortDirection::Input)],
            outputs: vec![],
            configuration: vec![],
        },
    ] {
        profile.insert(definition).unwrap();
    }
    (startup, profile)
}

#[test]
fn selected_canonical_back_changes_only_expansion_identity_and_records_exact_provenance() {
    let (mut startup, mut profile) = catalogs();
    startup
        .insert(KindSignature {
            kind: "test/high".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let high = KindDefinition {
        kind_id: kind_id("test/high"),
        kind_contract_revision: KindContractRevision::from("test/high@1"),
        inputs: vec![port("in", PortDirection::Input)],
        outputs: vec![port("out", PortDirection::Output)],
        configuration: vec![],
    };
    profile.insert(high.clone()).unwrap();

    let user = check_syntax_document(
        &parse_syntax_document(
            "form main {\n source: test/source\n high: test/high\n sink: test/sink\n source > high > sink\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let direct = expand_canonical_form(&user, "main", &profile).unwrap();

    let back_document = check_syntax_document(
        &parse_syntax_document(
            "form test/high (\n in: test/value > out: test/value\n) {\n leaf: test/pass\n in > leaf > out\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let mut backs = crate::CanonicalBackCatalog::new();
    backs.insert(&high, &back_document, "test/high").unwrap();
    let recursive =
        crate::expand_canonical_form_with_backs(&user, "main", &profile, &backs).unwrap();

    assert_eq!(direct.source_document_id, recursive.source_document_id);
    assert_eq!(direct.checked_form_id, recursive.checked_form_id);
    assert_ne!(direct.expanded_form_id, recursive.expanded_form_id);
    assert!(direct.realization_backs.is_empty());
    assert_eq!(recursive.realization_backs.len(), 1);
    assert_eq!(recursive.realization_backs[0].invocation_path, "main/high");
    assert_eq!(recursive.realization_backs[0].kind_id.as_str(), "test/high");
    assert_eq!(
        recursive.realization_backs[0].source_document_id,
        back_document.source_document_id
    );
    assert!(recursive
        .gears
        .iter()
        .any(|gear| gear.gear_id.as_str() == "main/high/leaf"));
    recursive.validate_expansion().unwrap();
}

#[test]
fn canonical_back_refuses_a_face_that_differs_from_the_high_level_kind() {
    let (startup, profile) = catalogs();
    let document = check_syntax_document(
        &parse_syntax_document("form wrong {\n leaf: test/source\n}\n"),
        &startup,
    )
    .unwrap();
    let mut backs = crate::CanonicalBackCatalog::new();
    let error = backs
        .insert(
            profile.get(&kind_id("test/pass")).unwrap(),
            &document,
            "wrong",
        )
        .unwrap_err();
    assert_eq!(
        error,
        crate::CanonicalBackError::FaceMismatch("test/pass".into())
    );
}

#[test]
fn exact_back_admission_refuses_stale_source_and_checked_form_identities() {
    let (startup, profile) = catalogs();
    let high = profile.get(&kind_id("test/pass")).unwrap();
    let document = check_syntax_document(
        &parse_syntax_document(
            "form test/pass (\n count: Count = 1\n in: test/value > out: test/value\n) {\n leaf: test/pass(count)\n in > leaf > out\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let checked_form_id = document.forms[0].checked_form_id.clone();

    let mut backs = crate::CanonicalBackCatalog::new();
    assert!(matches!(
        backs.insert_exact(
            high,
            &[StartupParameterSignature {
                name: "count".into(),
                value_type: "Count".into(),
                default: Some("1".into()),
            }],
            &document,
            "test/pass",
            &conduit_core::SourceDocumentId::from("stale-source"),
            &checked_form_id,
        ),
        Err(crate::CanonicalBackError::StaleSourceDocument { .. })
    ));
    assert!(matches!(
        backs.insert_exact(
            high,
            &[StartupParameterSignature {
                name: "count".into(),
                value_type: "Count".into(),
                default: Some("1".into()),
            }],
            &document,
            "test/pass",
            &document.source_document_id,
            &conduit_core::CheckedFormId::from("stale-form"),
        ),
        Err(crate::CanonicalBackError::StaleCheckedForm { .. })
    ));

    backs
        .insert_exact(
            high,
            &[StartupParameterSignature {
                name: "count".into(),
                value_type: "Count".into(),
                default: Some("1".into()),
            }],
            &document,
            "test/pass",
            &document.source_document_id,
            &checked_form_id,
        )
        .unwrap();
}

fn expand(source: &str, root: &str) -> crate::ExpandedCanonicalForm {
    let (startup, profile) = catalogs();
    let syntax = parse_syntax_document(source);
    let checked = check_syntax_document(&syntax, &startup).expect("source checks");
    expand_canonical_form(&checked, root, &profile).expect("source expands")
}

#[test]
fn parameterized_form_flattens_to_ordinary_primitive_graph() {
    let source = "form relay (\n count: Count = 1\n input: test/value > output: test/value\n) {\n pass: test/pass(count)\n input > pass > output\n}\n\nform main {\n source: test/source\n relay: relay(2)\n sink: test/sink\n source > relay > sink\n}\n";
    let expanded = expand(source, "main");

    assert_eq!(
        expanded
            .gears
            .iter()
            .map(|gear| gear.gear_id.as_str())
            .collect::<Vec<_>>(),
        ["main/relay/pass", "main/sink", "main/source"]
    );
    let pass = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "test/pass")
        .unwrap();
    assert_eq!(pass.configuration[0].value, ConfigurationValue::U64(2));
    assert_eq!(expanded.connections.len(), 2);
    assert_eq!(expanded.provenance[0].source_form, "relay");
    assert_ne!(
        expanded.checked_form_id.as_str(),
        expanded.expanded_form_id.as_str()
    );
}

#[test]
fn two_explicit_consumers_share_one_exact_expanded_pool_reference() {
    let source = "form chat/peer (\n recv: ChatMessage...| > send: ChatMessage...|\n) {\n}\n\nform consumer (\n members: Pool\n) {\n use: test/use-pool(members)\n}\n\nform room {\n pool peers: chat/peer(size = 2)\n left: consumer(peers)\n right: consumer(peers)\n}\n";
    let expanded = expand(source, "room");
    assert_eq!(expanded.shared_pools.len(), 1);
    let pool = &expanded.shared_pools[0];
    assert_eq!(pool.pool_id.as_str(), "room/peers");
    assert_eq!(pool.maximum_members, 2);
    assert_eq!(
        pool.consumers
            .iter()
            .map(|consumer| consumer.as_str())
            .collect::<Vec<_>>(),
        ["room/left/use", "room/right/use"]
    );
    assert!(expanded.gears.iter().all(|gear| {
        gear.pool_references == vec![conduit_core::SharedPoolId::from("room/peers")]
    }));
    expanded.validate_expansion().unwrap();

    let mut mutated = expanded;
    mutated.shared_pools[0].maximum_members = 3;
    assert!(mutated.validate_expansion().is_err());
}

#[test]
fn pool_name_is_not_ambiently_captured_by_nested_forms_or_graph_cords() {
    let (startup, profile) = catalogs();
    let ambient = parse_syntax_document(
        "form chat/peer {\n}\n\nform consumer {\n use: test/use-pool(peers)\n}\n\nform room {\n pool peers: chat/peer(size = 2)\n child: consumer\n}\n",
    );
    let checked = check_syntax_document(&ambient, &startup).unwrap();
    let diagnostic = expand_canonical_form(&checked, "room", &profile).unwrap_err();
    assert_eq!(diagnostic.code, "CND-FRM-041");

    let implicit = parse_syntax_document(
        "form chat/peer {\n}\n\nform room {\n pool peers: chat/peer(size = 2)\n source: test/source\n source > peers\n}\n",
    );
    let checked = check_syntax_document(&implicit, &startup).unwrap();
    let diagnostic = expand_canonical_form(&checked, "room", &profile).unwrap_err();
    assert_eq!(diagnostic.code, "CND-FRM-042");
}

#[test]
fn nested_expansion_and_source_reordering_have_deterministic_identity() {
    let first = "form inner (\n input: test/value > output: test/value\n) {\n pass: test/pass\n input > pass > output\n}\n\nform outer (\n input: test/value > output: test/value\n) {\n inner: inner\n input > inner > output\n}\n\nform main {\n source: test/source\n outer: outer\n sink: test/sink\n source > outer > sink\n}\n";
    let reordered = "form main {\n source > outer > sink\n sink: test/sink\n outer: outer\n source: test/source\n}\n\nform outer (\n input: test/value > output: test/value\n) {\n input > inner > output\n inner: inner\n}\n\nform inner (\n input: test/value > output: test/value\n) {\n input > pass > output\n pass: test/pass\n}\n";
    let first = expand(first, "main");
    let reordered = expand(reordered, "main");
    assert_eq!(first.checked_form_id, reordered.checked_form_id);
    assert_eq!(first.expanded_form_id, reordered.expanded_form_id);
    assert_eq!(first.gears, reordered.gears);
    assert_eq!(first.connections, reordered.connections);
}

#[test]
fn two_uses_share_one_form_definition_but_have_distinct_occurrence_paths() {
    let source = "form relay (\n input: test/value > output: test/value\n) {\n pass: test/pass\n input > pass > output\n}\n\nform main {\n source: test/source\n left: relay\n right: relay\n left_sink: test/sink\n right_sink: test/sink\n source > left > left_sink\n source > right > right_sink\n}\n";
    let expanded = expand(source, "main");
    let relay_gears = expanded
        .provenance
        .iter()
        .filter(|item| item.source_form == "relay")
        .collect::<Vec<_>>();

    assert_eq!(relay_gears.len(), 2);
    assert_eq!(relay_gears[0].source_gear, "pass");
    assert_eq!(relay_gears[1].source_gear, "pass");
    assert_eq!(relay_gears[0].form_path, ["main", "left"]);
    assert_eq!(relay_gears[1].form_path, ["main", "right"]);
    assert_ne!(relay_gears[0].gear_id, relay_gears[1].gear_id);
    expanded.validate_expansion().unwrap();
}

#[test]
fn recursion_and_expansion_depth_fail_with_distinct_diagnostics() {
    let (startup, profile) = catalogs();
    let recursive = parse_syntax_document("form a {\n child: b\n}\n\nform b {\n child: a\n}\n");
    let checked = check_syntax_document(&recursive, &startup).unwrap();
    let error = expand_canonical_form(&checked, "a", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-035");
    assert!(error.message.contains("a > b > a"));

    let mut source = String::new();
    for index in 0..=crate::MAXIMUM_FORM_NESTING_DEPTH + 1 {
        source.push_str(&format!("form f{index} {{\n"));
        if index <= crate::MAXIMUM_FORM_NESTING_DEPTH {
            source.push_str(&format!(" child: f{}\n", index + 1));
        }
        source.push_str("}\n\n");
    }
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let error = expand_canonical_form(&checked, "f0", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-034");
}

#[test]
fn reusable_form_without_declared_shorthand_requires_named_port() {
    let source = "form source (\n value: test/value >\n) {\n primitive: test/source\n primitive > value\n}\n\nform main {\n source: source\n sink: test/sink\n source > sink\n}\n";
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let error = expand_canonical_form(&checked, "main", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-044");
}

#[test]
fn primitive_contract_bounds_and_face_types_fail_closed() {
    let bounded = "form main {\n source: test/source\n pass: test/pass(9)\n sink: test/sink\n source > pass > sink\n}\n";
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(bounded), &startup).unwrap();
    assert_eq!(
        expand_canonical_form(&checked, "main", &profile)
            .unwrap_err()
            .code,
        "CND-FRM-040"
    );

    let wrong_face = "form relay (\n input: wrong/value > output: wrong/value\n) {\n pass: test/pass\n input > pass > output\n}\n\nform main {\n source: test/source\n relay: relay\n sink: test/sink\n source > relay > sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(wrong_face), &startup).unwrap();
    assert_eq!(
        expand_canonical_form(&checked, "main", &profile)
            .unwrap_err()
            .code,
        "CND-FRM-045"
    );
}

#[test]
fn public_input_fanout_flattens_to_explicit_ordinary_connections() {
    let source = "form fan (\n > input: test/value\n) {\n left: test/pass\n right: test/pass\n input > left\n input > right\n}\n\nform main {\n source: test/source\n fan: fan\n source > fan.input\n}\n";
    let expanded = expand(source, "main");
    assert_eq!(expanded.connections.len(), 2);
    assert!(expanded.connections.iter().all(|connection| {
        connection.source_gear_id.as_str() == "main/source"
            && matches!(
                connection.sink_gear_id.as_str(),
                "main/fan/left" | "main/fan/right"
            )
    }));
}

#[test]
fn expanded_identity_rejects_graph_contract_and_provenance_mutation() {
    let source = "form main {\n source: test/source\n sink: test/sink\n source > sink\n}\n";
    let baseline = expand(source, "main");

    let mut gear = baseline.clone();
    gear.gears[0].kind_contract_revision = KindContractRevision::from("mutated@1");
    assert_eq!(gear.validate_expansion().unwrap_err().code, "CND-FRM-049");

    let mut cord = baseline.clone();
    cord.connections[0].sink_port_id = port_id("mutated");
    assert_eq!(cord.validate_expansion().unwrap_err().code, "CND-FRM-049");

    let mut provenance = baseline;
    provenance.provenance[0].source_gear = "substituted".into();
    assert_eq!(
        provenance.validate_expansion().unwrap_err().code,
        "CND-FRM-049"
    );

    let mut span = expand(source, "main");
    span.provenance[0].source_span.start += 1;
    assert_eq!(span.validate_expansion().unwrap_err().code, "CND-FRM-049");
}

#[test]
fn inline_reusable_and_primitive_gears_expand_without_a_parallel_path() {
    let source = "form relay (\n input: test/value > output: test/value\n) {\n input > test/pass > output\n}\n\nform main {\n test/source > relay() > test/sink\n}\n";
    let expanded = expand(source, "main");
    assert_eq!(expanded.gears.len(), 3);
    assert_eq!(expanded.connections.len(), 2);
    assert!(expanded
        .provenance
        .iter()
        .all(|row| row.source_gear.starts_with("inline-")));
}

#[test]
fn face_binding_preserves_flow_closure_and_current_observation_contracts() {
    let mut startup = StartupCatalog::new();
    for gear in ["state/count", "test/ticks", "test/current"] {
        startup
            .insert(KindSignature {
                kind: gear.into(),
                startup_parameters: vec![],
            })
            .unwrap();
    }
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindDefinition {
            kind_id: kind_id("state/count"),
            kind_contract_revision: KindContractRevision::from("state/count@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("bump"),
                value_kind: kind_id("value/tick@1"),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Flow { closes: true },
            }],
            outputs: vec![PortDescriptor {
                port_id: port_id("value"),
                value_kind: kind_id("value/count@1"),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Current,
            }],
            configuration: vec![],
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: kind_id("test/ticks"),
            kind_contract_revision: KindContractRevision::from("test/ticks@1"),
            inputs: vec![],
            outputs: vec![PortDescriptor {
                port_id: port_id("tick"),
                value_kind: kind_id("value/tick@1"),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Flow { closes: true },
            }],
            configuration: vec![],
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: kind_id("test/current"),
            kind_contract_revision: KindContractRevision::from("test/current@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("value"),
                value_kind: kind_id("value/count@1"),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Current,
            }],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    let source = "form count (\n    bump: Tick...| > value: $Count\n) {\n    gear: state/count\n    bump > gear.bump\n    gear.value > value\n}\n\nform main {\n    ticks: test/ticks\n    count: count\n    show: test/current\n    ticks > count > show\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "main", &profile).unwrap();
    let count = checked
        .forms
        .iter()
        .find(|form| form.name == "count")
        .unwrap();
    let state = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "state/count")
        .unwrap();
    assert_eq!(count.checked_face(), state.checked_face());

    let mismatched = source.replace("Tick...|", "Tick...");
    let checked = check_syntax_document(&parse_syntax_document(&mismatched), &startup).unwrap();
    assert_eq!(
        expand_canonical_form(&checked, "main", &profile)
            .unwrap_err()
            .code,
        "CND-FRM-045"
    );
}
