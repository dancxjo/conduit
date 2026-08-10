use crate::{
    gui::{draw_patchbay, GuiAction, LifecycleContext},
    icon::Icon,
    render::BACKGROUND,
};
use patchbay_model::{FormEditor, PatchbayGraph, PHOSPHOR_THEME};
use std::path::PathBuf;

fn graph() -> PatchbayGraph {
    let editor = FormEditor::from_source(
        PathBuf::from("count.conduit"),
        include_str!("../../../examples/count.conduit").into(),
    )
    .unwrap();
    PatchbayGraph::from_expanded(&editor.expand_form("count-demo").unwrap()).unwrap()
}

#[test]
fn icon_table_is_finite_and_every_icon_has_an_accessibility_name() {
    assert_eq!(Icon::ALL.len(), 25);
    assert!(Icon::ALL
        .iter()
        .all(|icon| !icon.accessibility_name().is_empty()));
}

#[test]
fn patchbay_draws_nodes_ports_cords_panels_and_bounded_hit_targets() {
    let graph = graph();
    let mut pixels = vec![BACKGROUND; 1100 * 720];
    let targets = draw_patchbay(
        &mut pixels,
        1100,
        720,
        &graph,
        None,
        &LifecycleContext::default(),
    );
    assert_eq!(
        targets.len(),
        3 + graph.gears.len()
            + graph.cords.len()
            + graph
                .gears
                .iter()
                .map(|gear| gear.inputs.len() + gear.outputs.len())
                .sum::<usize>()
    );
    assert!(pixels.contains(&PHOSPHOR_THEME.structure_primary.packed_rgb()));
    assert!(pixels.contains(&PHOSPHOR_THEME.emphasis.packed_rgb()));
    assert!(targets
        .iter()
        .any(|target| matches!(&target.action, GuiAction::SelectSubject(identity) if identity.starts_with("cord/"))));
    assert!(targets
        .iter()
        .any(|target| matches!(&target.action, GuiAction::SelectSubject(identity) if identity.starts_with("port/"))));
    assert!(targets
        .iter()
        .any(|target| target.action == GuiAction::OpenNextForm && target.contains(20.0, 250.0)));
    assert!(targets
        .iter()
        .any(|target| target.action == GuiAction::SaveForm && target.contains(20.0, 282.0)));
    assert!(targets.iter().any(|target| {
        target.action == GuiAction::ToggleLinearView && target.contains(20.0, 314.0)
    }));
}

#[test]
fn resize_clipping_and_selection_cannot_touch_guard_pixels_or_graph_identity() {
    let graph = graph();
    let identities = (
        graph.source_document_id.clone(),
        graph.checked_form_id.clone(),
        graph.expanded_form_id.clone(),
    );
    let selected = graph.cords[0].identity.clone();
    let guard = 0x00aa_55aa;
    let mut storage = vec![guard; 102];
    draw_patchbay(
        &mut storage[1..101],
        10,
        10,
        &graph,
        Some(&selected),
        &LifecycleContext::default(),
    );
    assert_eq!(storage[0], guard);
    assert_eq!(storage[101], guard);
    assert_eq!(
        identities,
        (
            graph.source_document_id,
            graph.checked_form_id,
            graph.expanded_form_id
        )
    );
}
