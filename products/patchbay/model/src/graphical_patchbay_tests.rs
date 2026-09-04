use crate::{
    FormEditor, PatchbayGraph, PatchbayGraphError, PatchbayPortCompatibility, PatchbaySubjectKind,
    MAX_PATCHBAY_GEARS, MAX_PATCHBAY_PORTS, MAX_PATCHBAY_SUBJECTS,
};
use std::path::PathBuf;

fn count_graph() -> PatchbayGraph {
    let editor = FormEditor::from_source(
        PathBuf::from("count.conduit"),
        include_str!("../../../../forms/count/main.conduit").into(),
    )
    .expect("canonical example");
    let expanded = editor.expand_form("count-demo").expect("expanded example");
    PatchbayGraph::from_expanded(&expanded).expect("finite graph")
}

fn composition_graph() -> PatchbayGraph {
    FormEditor::from_source(
        PathBuf::from("greet.conduit"),
        include_str!("../../../../forms/greet/main.conduit").into(),
    )
    .unwrap()
    .patchbay_graph_for_authoring("default-welcome")
    .unwrap()
}

#[test]
fn composition_admission_combines_gear_port_and_subject_bounds_atomically() {
    let graph = composition_graph();
    let port_count = graph.face_inputs.len()
        + graph.face_outputs.len()
        + graph
            .gears
            .iter()
            .map(|gear| gear.inputs.len() + gear.outputs.len())
            .sum::<usize>()
        + graph
            .compositions
            .iter()
            .map(|composition| composition.inputs.len() + composition.outputs.len())
            .sum::<usize>();
    assert!(graph.gears.len() + graph.compositions.len() <= MAX_PATCHBAY_GEARS);
    assert!(port_count <= MAX_PATCHBAY_PORTS);
    assert_eq!(graph.subject_count(), graph.subject_identities().count());
    assert!(graph.subject_count() <= MAX_PATCHBAY_SUBJECTS);

    let composition = graph.compositions[0].clone();
    let mut gear_full = graph.clone();
    let retained = gear_full.gears[0].clone();
    while gear_full.gears.len() + gear_full.compositions.len() < MAX_PATCHBAY_GEARS {
        gear_full.gears.push(retained.clone());
    }
    let before = gear_full.compositions.clone();
    assert_eq!(
        gear_full.admit_composition(composition.clone()),
        Err(PatchbayGraphError::TooManyGears)
    );
    assert_eq!(gear_full.compositions, before);

    let mut port_full = graph.clone();
    let mut oversized = composition;
    let port = oversized.inputs[0].clone();
    oversized.inputs = vec![port; MAX_PATCHBAY_PORTS];
    let before = port_full.compositions.clone();
    assert_eq!(
        port_full.admit_composition(oversized),
        Err(PatchbayGraphError::TooManyPorts)
    );
    assert_eq!(port_full.compositions, before);
}

#[test]
fn canonical_count_example_projects_three_gears_typed_ports_and_exact_cords() {
    let graph = count_graph();
    assert_eq!(graph.gears.len(), 3);
    assert!(!graph.cords.is_empty());
    assert!(graph
        .gears
        .iter()
        .flat_map(|gear| &gear.inputs)
        .all(|port| port.identity.contains("/input/")));
    assert!(graph
        .gears
        .iter()
        .flat_map(|gear| &gear.outputs)
        .all(|port| port.identity.contains("/output/")));
    for cord in &graph.cords {
        assert!(graph
            .gears
            .iter()
            .flat_map(|gear| &gear.outputs)
            .any(|port| port.identity == cord.source_port));
        assert!(graph
            .gears
            .iter()
            .flat_map(|gear| &gear.inputs)
            .any(|port| port.identity == cord.sink_port));
    }
}

#[test]
fn connection_compatibility_comes_from_exact_admitted_port_contracts() {
    let graph = count_graph();
    let cord = &graph.cords[0];
    assert_eq!(
        graph.connection_compatibility(&cord.source_port, &cord.sink_port),
        PatchbayPortCompatibility::DuplicateCord
    );
    assert_eq!(
        graph.connection_compatibility(&cord.sink_port, &cord.source_port),
        PatchbayPortCompatibility::InvalidDirection
    );
    assert_eq!(
        graph.connection_compatibility("port/invented/output/value", &cord.sink_port),
        PatchbayPortCompatibility::UnknownPort
    );

    let mut retyped = graph.clone();
    let sink = retyped
        .gears
        .iter_mut()
        .flat_map(|gear| &mut gear.inputs)
        .find(|port| port.identity == cord.sink_port)
        .unwrap();
    let source_kind = graph
        .gears
        .iter()
        .flat_map(|gear| &gear.outputs)
        .find(|port| port.identity == cord.source_port)
        .unwrap()
        .descriptor
        .value_kind
        .clone();
    sink.descriptor.value_kind = conduit_core::KindId::from("value/incompatible@1");
    assert_eq!(
        retyped.connection_compatibility(&cord.source_port, &cord.sink_port),
        PatchbayPortCompatibility::IncompatibleInfo {
            source: source_kind,
            sink: conduit_core::KindId::from("value/incompatible@1"),
        }
    );

    let composition = composition_graph();
    let hello = &composition.compositions[0];
    let bound_source = &hello.output_bindings[0].internal_port;
    let existing = composition
        .cords
        .iter()
        .find(|cord| &cord.source_port == bound_source)
        .unwrap();
    assert_eq!(
        composition.connection_compatibility(&hello.outputs[0].identity, &existing.sink_port),
        PatchbayPortCompatibility::DuplicateCord
    );
}

#[test]
fn inspector_can_only_report_a_subject_from_the_typed_projection() {
    let graph = count_graph();
    let cord = &graph.cords[0];
    let inspection = graph.inspect(&cord.identity).expect("exact Cord");
    assert_eq!(inspection.subject_kind, PatchbaySubjectKind::Cord);
    assert!(inspection
        .exact_facts
        .iter()
        .any(|fact| fact.starts_with("Info ")));
    assert_eq!(
        graph.inspect("renderer-invented/subject"),
        Err(PatchbayGraphError::UnknownSubject)
    );
}

#[test]
fn selection_candidate_requires_the_exact_graph_basis_and_an_admitted_subject() {
    let graph = count_graph();
    let identity = graph.subject_identities().nth(1).unwrap();
    let candidate = graph.subject_ref(identity).unwrap();
    assert_eq!(graph.resolve_subject_ref(&candidate), Ok(1));

    let mut stale = candidate.clone();
    stale.expanded_form_id = conduit_core::ExpandedFormId::from("expanded/stale");
    assert_eq!(
        graph.resolve_subject_ref(&stale),
        Err(PatchbayGraphError::StaleGraphBasis)
    );

    let mut invented = candidate;
    invented.subject_identity = "renderer-invented/subject".into();
    assert_eq!(
        graph.resolve_subject_ref(&invented),
        Err(PatchbayGraphError::UnknownSubject)
    );
}

#[test]
fn missing_or_retyped_cord_endpoints_fail_closed() {
    let editor = FormEditor::from_source(
        PathBuf::from("count.conduit"),
        include_str!("../../../../forms/count/main.conduit").into(),
    )
    .unwrap();
    let mut expanded = editor.expand_form("count-demo").unwrap();
    expanded.connections[0].source_port_id = conduit_core::PortId::from("invented");
    assert_eq!(
        PatchbayGraph::from_expanded(&expanded),
        Err(PatchbayGraphError::MissingCordEndpoint)
    );

    let mut expanded = editor.expand_form("count-demo").unwrap();
    expanded.connections[0].value_kind = conduit_core::KindId::from("value/invented@1");
    assert_eq!(
        PatchbayGraph::from_expanded(&expanded),
        Err(PatchbayGraphError::CordContractMismatch)
    );
}

#[test]
fn selection_and_layout_inputs_do_not_enter_canonical_identity() {
    let graph = count_graph();
    let identities = (
        graph.source_document_id.clone(),
        graph.checked_form_id.clone(),
        graph.expanded_form_id.clone(),
    );
    for subject in graph.subject_identities() {
        graph.inspect(subject).expect("admitted subject");
    }
    assert_eq!(
        identities,
        (
            graph.source_document_id,
            graph.checked_form_id,
            graph.expanded_form_id
        )
    );
}
