//! CPU projection boundary checks, not browser input or owner acknowledgement.
use crate::{
    portable_content::ContentBuilder, portable_graph_projection::append_exact_graph, PatchbayGraph,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_face::{FacePropertyValue, FaceRelationshipKind, FaceRole};

#[test]
fn root_fore_ports_are_real_subjects_and_missing_endpoints_are_not_invented() {
    let mut profile = conduit_plot::ProfileCatalog::new();
    let port = |name: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id("value/count"),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    profile
        .insert_kind(Kind {
            kind_id: kind_id("test/pass"),
            kind_contract_revision: "test@1".into(),
            startup_parameters: vec![],
            shorthand: None,
            inputs: vec![port("in", PortDirection::Input)],
            outputs: vec![port("out", PortDirection::Output)],
            configuration: vec![],
            semantic_laws: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 64,
            },
        })
        .unwrap();
    let source = "plot main (\n input: Count >> output: Count\n) {\n pass: test/pass\n input >> pass >> output\n}\n";
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(source),
        &profile.startup_catalog().unwrap(),
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "main", &profile).unwrap();
    let graph = PatchbayGraph::from_authoring(&expanded).unwrap();
    assert_eq!(
        (graph.front_inputs.len(), graph.front_outputs.len()),
        (1, 1)
    );
    for missing in [false, true] {
        let mut graph = graph.clone();
        if missing {
            graph.cords[0].sink_port = "absent/port".into();
        }
        let mut content = ContentBuilder::new();
        let plot = content.subject_with_identity(
            format!("plot/{}", graph.checked_plot_id.as_str()),
            FaceRole::Plot,
            graph.plot_name.clone(),
        );
        append_exact_graph(&plot, &graph, None, None, &mut content);
        // Native's installed graph projection delegates to this complete shared
        // projection. Compare meaning, allowing browser labels/augmentation.
        let (mut subjects, mut relationships, mut properties, mut text, mut disclosures) =
            (vec![], vec![], vec![], vec![], vec![]);
        patchbay_application::graph_face::append(
            &graph,
            "body/root",
            &mut patchbay_application::graph_face::Content {
                subjects: &mut subjects,
                relationships: &mut relationships,
                properties: &mut properties,
                texts: &mut text,
                disclosures: &mut disclosures,
            },
        );
        let identities = |subjects: &[conduit_face::FaceSubject]| {
            subjects
                .iter()
                .filter(|subject| {
                    matches!(
                        subject.role,
                        FaceRole::Gear | FaceRole::Port | FaceRole::Cord
                    )
                })
                .map(|subject| subject.identity.clone())
                .collect::<std::collections::BTreeSet<_>>()
        };
        let native_ids = identities(&subjects);
        assert_eq!(native_ids, identities(&content.subjects));
        for property in properties
            .iter()
            .filter(|property| native_ids.contains(&property.subject))
        {
            assert!(
                content.properties.contains(property),
                "missing shared graph fact: {property:?}"
            );
        }
        for relation in relationships.iter().filter(|relation| {
            (native_ids.contains(&relation.source) || relation.source == plot)
                && native_ids.contains(&relation.target)
        }) {
            assert!(
                content.relationships.contains(relation),
                "missing shared topology: {relation:?}"
            );
        }
        for port in graph.front_inputs.iter().chain(&graph.front_outputs) {
            assert!(content.subjects.iter().any(|subject| subject.identity
                == format!("port/{}", port.identity)
                && subject.role == FaceRole::Port));
        }
        for cord in &graph.cords {
            let id = format!("cord/{}", cord.identity);
            for (name, endpoint) in [
                ("source-port", &cord.source_port),
                ("sink-port", &cord.sink_port),
            ] {
                assert!(content
                    .properties
                    .iter()
                    .any(|property| property.subject == id
                        && property.name == name
                        && property.value == FacePropertyValue::Identity(endpoint.clone())));
            }
            let endpoints: Vec<_> = content
                .relationships
                .iter()
                .filter(|relation| {
                    relation.source == id && relation.kind == FaceRelationshipKind::Connects
                })
                .map(|relation| relation.target.as_str())
                .collect();
            let expected: Vec<_> = [&cord.source_port, &cord.sink_port]
                .into_iter()
                .filter(|endpoint| endpoint.as_str() != "absent/port")
                .map(|endpoint| format!("port/{endpoint}"))
                .collect();
            assert_eq!(endpoints, expected);
        }
        assert!(!content
            .subjects
            .iter()
            .any(|subject| subject.identity == "port/absent/port"));
    }
}
