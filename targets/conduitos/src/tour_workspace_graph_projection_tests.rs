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
