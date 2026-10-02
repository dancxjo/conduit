use patchbay_application::PatchbayLayout;
use patchbay_graph::PatchbayGraph;
use std::path::PathBuf;

use crate::PlotEditor;

#[test]
fn movement_grouping_and_round_trip_never_change_graph_identity() {
    let editor = PlotEditor::from_source(
        PathBuf::from("count.conduit"),
        include_str!("../../../../../plots/count/main.conduit").into(),
    )
    .unwrap();
    let graph = PatchbayGraph::from_expanded(&editor.expand_plot("count-demo").unwrap()).unwrap();
    let identities = (
        graph.source_document_id.clone(),
        graph.checked_plot_id.clone(),
        graph.expanded_plot_id.clone(),
    );
    let gear = graph.subject_ref(&graph.gears[0].identity).unwrap();
    let mut layout = PatchbayLayout::default();
    layout.move_gear(&graph, &gear, 420, 180).unwrap();
    layout
        .group_gear(&graph, &gear, Some("sensing".into()))
        .unwrap();
    let encoded = serde_json::to_vec(&layout).unwrap();
    let reopened: PatchbayLayout = serde_json::from_slice(&encoded).unwrap();
    reopened.validate().unwrap();
    assert_eq!(reopened.position(&gear.subject_identity), Some((420, 180)));
    assert_eq!(
        identities,
        (
            graph.source_document_id,
            graph.checked_plot_id,
            graph.expanded_plot_id
        )
    );
}

#[test]
fn cord_waypoint_round_trip_is_bounded_presentation_only() {
    let editor = PlotEditor::from_source(
        PathBuf::from("route.conduit"),
        "plot route {\n    literal: text/literal(\"hello\")\n    upper: text/upper\n    literal.text >> upper.source\n}\n".into(),
    )
    .unwrap();
    let graph = PatchbayGraph::from_expanded(&editor.expand_plot("route").unwrap()).unwrap();
    let identities = (
        graph.source_document_id.clone(),
        graph.checked_plot_id.clone(),
        graph.expanded_plot_id.clone(),
        graph.cords[0].identity.clone(),
    );
    let cord = graph.subject_ref(&graph.cords[0].identity).unwrap();
    let mut layout = PatchbayLayout::default();
    layout.route_cord(&graph, &cord, 420, 240).unwrap();
    let encoded = serde_json::to_vec(&layout).unwrap();
    let reopened: PatchbayLayout = serde_json::from_slice(&encoded).unwrap();
    reopened.validate().unwrap();
    assert_eq!(
        reopened.cord_route(&graph.cords[0].source_port, &graph.cords[0].sink_port),
        Some((420, 240))
    );
    assert_eq!(
        identities,
        (
            graph.source_document_id,
            graph.checked_plot_id,
            graph.expanded_plot_id,
            graph.cords[0].identity.clone()
        )
    );
}
