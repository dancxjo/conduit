use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConnectionTrack, Kind, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document,
    validate_connection_contract, ProfileCatalog,
};
use patchbay_graph::{PatchbayGraph, PatchbayPortCompatibility};

fn port(direction: PortDirection, kind: &str, temporal: PortTemporal) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("port"),
        value_kind: kind_id(kind),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

fn catalog(source: &PortDescriptor, sink: &PortDescriptor) -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    for (name, input, output) in [
        ("test/source", vec![], vec![source.clone()]),
        ("test/sink", vec![sink.clone()], vec![]),
    ] {
        catalog
            .insert_kind(Kind {
                kind_id: kind_id(name),
                kind_contract_revision: "test@1".into(),
                startup_parameters: vec![],
                shorthand: None,
                inputs: input,
                outputs: output,
                configuration: vec![],
                semantic_laws: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: 64,
                },
            })
            .unwrap();
    }
    catalog
}

#[test]
fn cord_identity_survives_insertion_and_reordering() {
    let source = port(PortDirection::Output, "value/count", PortTemporal::Value);
    let sink = port(PortDirection::Input, "value/count", PortTemporal::Value);
    let catalog = catalog(&source, &sink);
    let startup = catalog.startup_catalog().unwrap();
    let graph = |cords: &str| {
        let text = format!(
            "plot main {{\n source: test/source\n sink: test/sink\n other: test/sink\n {cords}\n}}\n"
        );
        let checked = check_syntax_document(&parse_syntax_document(&text), &startup).unwrap();
        PatchbayGraph::from_expanded(&expand_canonical_plot(&checked, "main", &catalog).unwrap())
            .unwrap()
    };
    let original = graph("source.port >> sink.port");
    let identity = &original.cords[0].identity;
    for text in [
        "source.port >> other.port\n source.port >> sink.port",
        "source.port >> sink.port\n source.port >> other.port",
    ] {
        let changed = graph(text);
        let retained = changed
            .cords
            .iter()
            .find(|cord| {
                cord.source_port == original.cords[0].source_port
                    && cord.sink_port == original.cords[0].sink_port
            })
            .unwrap();
        assert_eq!(&retained.identity, identity);
        assert_ne!(changed.cords[0].identity, changed.cords[1].identity);
    }
}

#[test]
fn payload_matrix_has_exact_source_checker_parity() {
    let temporal = [
        PortTemporal::Value,
        PortTemporal::Current,
        PortTemporal::Flow { closes: true },
        PortTemporal::Flow { closes: false },
    ];
    for source_time in temporal {
        for sink_time in temporal {
            for sink_kind in ["value/count", "value/text"] {
                let source = port(PortDirection::Output, "value/count", source_time);
                let sink = port(PortDirection::Input, sink_kind, sink_time);
                let catalog = catalog(&source, &sink);
                let startup = catalog.startup_catalog().unwrap();
                let text = "plot main {\n source: test/source\n sink: test/sink\n}\n";
                let checked =
                    check_syntax_document(&parse_syntax_document(text), &startup).unwrap();
                let expanded = expand_canonical_plot(&checked, "main", &catalog).unwrap();
                let graph = PatchbayGraph::from_expanded(&expanded).unwrap();
                let source_id = &graph
                    .gears
                    .iter()
                    .find(|gear| gear.kind_id.as_str() == "test/source")
                    .unwrap()
                    .outputs[0]
                    .identity;
                let sink_id = &graph
                    .gears
                    .iter()
                    .find(|gear| gear.kind_id.as_str() == "test/sink")
                    .unwrap()
                    .inputs[0]
                    .identity;
                let tool = graph.connection_compatibility(source_id, sink_id);
                let connected = text.replace("\n}", "\n source.port >> sink.port\n}");
                let checked =
                    check_syntax_document(&parse_syntax_document(&connected), &startup).unwrap();
                let source_result = expand_canonical_plot(&checked, "main", &catalog);
                assert_eq!(
                    tool == PatchbayPortCompatibility::Compatible,
                    source_result.is_ok()
                );
                assert_eq!(
                    validate_connection_contract(&source, &sink, ConnectionTrack::Payload).is_ok(),
                    source_result.is_ok()
                );
            }
        }
    }
}

#[test]
fn projected_terminal_tracks_use_the_exact_checker_contract() {
    for (track, suffix, sink_kind) in [
        (ConnectionTrack::NormalClose, "|", "value/unit"),
        (ConnectionTrack::Quiescence, ";", "value/unit"),
        (ConnectionTrack::AbnormalTerminal, "!", "value/bool"),
    ] {
        let mut source = port(
            PortDirection::Output,
            "value/count",
            PortTemporal::Flow { closes: true },
        );
        source.abnormal_kind = Some(kind_id("value/bool"));
        for accepted in [true, false] {
            let sink = port(
                PortDirection::Input,
                if accepted { sink_kind } else { "value/text" },
                PortTemporal::Value,
            );
            let catalog = catalog(&source, &sink);
            let startup = catalog.startup_catalog().unwrap();
            let text = format!("plot main {{\n source: test/source\n sink: test/sink\n source.port{suffix} >> sink.port\n}}\n");
            let checked = check_syntax_document(&parse_syntax_document(&text), &startup).unwrap();
            let expanded = expand_canonical_plot(&checked, "main", &catalog);
            assert_eq!(expanded.is_ok(), accepted);
            assert_eq!(
                validate_connection_contract(&source, &sink, track).is_ok(),
                accepted
            );
            if let Ok(expanded) = expanded {
                assert!(PatchbayGraph::from_expanded(&expanded).is_ok());
            }
        }
    }
}

#[test]
fn fore_boundaries_follow_source_binding_law_not_internal_cord_law() {
    let source = port(PortDirection::Output, "value/count", PortTemporal::Value);
    let sink = port(
        PortDirection::Input,
        "value/count",
        PortTemporal::Flow { closes: false },
    );
    let catalog = catalog(&source, &sink);
    let startup = catalog.startup_catalog().unwrap();
    let text = "plot main (\n >> input: Count\n) {\n sink: test/sink\n input >> sink.port\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(text), &startup).unwrap();
    let authoring =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "main", &catalog).unwrap();
    let mut graph = PatchbayGraph::from_authoring(&authoring).unwrap();
    graph.cords.clear();
    assert_eq!(
        graph.connection_compatibility(
            &graph.front_inputs[0].identity,
            &graph.gears[0].inputs[0].identity
        ),
        PatchbayPortCompatibility::Compatible,
    );
    assert!(validate_connection_contract(&source, &sink, ConnectionTrack::Payload).is_err());
}
