//! Shared checked graph consumption, independent of storage order.
use super::*;

fn render(graph: &patchbay_graph::PatchbayGraph) -> GraphicsScene {
    let state = TourWorkspaceState::canonical(1, TourWorkspacePhase::PatchbayOpen);
    super::super::scene_with_graph(1280, 800, &state, graph, None).unwrap()
}

fn paths(scene: &GraphicsScene) -> alloc::vec::Vec<GraphicsPath> {
    scene
        .commands()
        .iter()
        .filter_map(GraphicsCommand::path_geometry)
        .collect()
}

#[test]
fn gear_storage_order_cannot_rewire_or_reposition_cards() {
    let mut graph = super::super::canonical_graph().unwrap();
    let original = render(&graph);
    graph.gears.reverse();
    assert_eq!(render(&graph), original);
}

#[test]
fn removed_cord_is_not_replaced_by_an_assumed_adjacent_connection() {
    let mut graph = super::super::canonical_graph().unwrap();
    let original = paths(&render(&graph));
    assert_eq!(original.len(), 2);
    graph.cords.remove(0);
    assert_eq!(paths(&render(&graph)), original[1..]);
    graph.cords.clear();
    assert!(paths(&render(&graph)).is_empty());
}

#[test]
fn cord_storage_order_preserves_exact_endpoint_geometry() {
    let mut graph = super::super::canonical_graph().unwrap();
    let mut original = paths(&render(&graph));
    original.reverse();
    graph.cords.reverse();
    assert_eq!(paths(&render(&graph)), original);
}

#[test]
fn tour_viewport_refuses_a_graph_with_an_unrepresented_gear() {
    let mut graph = super::super::canonical_graph().unwrap();
    graph.gears.pop();
    let state = TourWorkspaceState::canonical(1, TourWorkspacePhase::PatchbayOpen);
    assert_eq!(
        super::super::scene_with_graph(1280, 800, &state, &graph, None),
        Err(TourWorkspaceSceneRefusal::Graph),
    );
}

#[test]
fn configured_literal_is_visible_without_claiming_an_observed_output() {
    let graph = super::super::canonical_graph().unwrap();
    let scene = render(&graph);
    let heading = scene
        .commands()
        .iter()
        .position(|command| command.payload().starts_with("words\n"))
        .unwrap();
    let body = scene.commands()[heading + 1].payload();
    assert!(body.contains("Configured value\n\"hello\""));
    assert!(body.contains("out text\nvalue/text@1\n= unobserved"));
    assert!(!body.contains("= \"hello\""));
}

#[test]
fn configured_preview_comes_from_checked_source_and_remains_bounded() {
    let source = conduit_tour_model::CANONICAL_SOURCE.replace(
        "hello",
        "a longer configured literal than fits a card preview",
    );
    let form =
        crate::ordinary_form::checked_expanded_text_form_named(&source, "meet-one-gear").unwrap();
    let graph = patchbay_graph::PatchbayGraph::from_expanded(&form).unwrap();
    let scene = render(&graph);
    let body = scene
        .commands()
        .iter()
        .find(|command| command.payload().contains("Configured value"))
        .unwrap()
        .payload();
    let value = body.split("Configured value\n").nth(1).unwrap();
    assert!(value.starts_with("\"a longer configured"));
    assert!(value.ends_with("..."));
    assert!(value.len() <= 26);
    let original = super::super::canonical_graph().unwrap();
    assert_eq!(paths(&scene), paths(&render(&original)));
}
