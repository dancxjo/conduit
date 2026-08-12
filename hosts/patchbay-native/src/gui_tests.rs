use crate::{
    gui::{draw_patchbay, GuiAction, LifecycleContext, PatchbayViewContext},
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
        PatchbayViewContext {
            selected: None,
            breadcrumb: "",
            lifecycle: &LifecycleContext::default(),
            palette_query: "",
            presentation_layout: &Default::default(),
            realization_plan: None,
            realization_hosts: &[],
        },
    );
    assert_eq!(
        targets.len(),
        3 + patchbay_model::GearPalette::standard()
            .unwrap()
            .entries()
            .len()
            + graph.gears.len()
            + graph.gears.len()
            + graph.cords.len()
            + graph
                .gears
                .iter()
                .map(|gear| gear.inputs.len() + gear.outputs.len())
                .sum::<usize>()
            + graph
                .gears
                .iter()
                .flat_map(|gear| &gear.controls)
                .map(|control| match control.kind {
                    patchbay_model::FaceControlKind::Number { .. }
                    | patchbay_model::FaceControlKind::ScalarNumber { .. }
                    | patchbay_model::FaceControlKind::Range { .. } => 2,
                    patchbay_model::FaceControlKind::BooleanChoice { .. }
                    | patchbay_model::FaceControlKind::TextChoice { .. }
                    | patchbay_model::FaceControlKind::ShortText { .. } => 1,
                })
                .sum::<usize>()
    );
    assert!(pixels.contains(&PHOSPHOR_THEME.structure_primary.packed_rgb()));
    assert!(pixels.contains(&PHOSPHOR_THEME.emphasis.packed_rgb()));
    assert!(targets
        .iter()
        .any(|target| matches!(&target.action, GuiAction::SelectSubject(subject) if subject.subject_identity.starts_with("cord/") && subject.expanded_form_id == graph.expanded_form_id)));
    assert!(targets
        .iter()
        .any(|target| matches!(&target.action, GuiAction::SelectSubject(subject) if subject.subject_identity.starts_with("port/") && subject.expanded_form_id == graph.expanded_form_id)));
    assert!(targets
        .iter()
        .any(|target| target.action == GuiAction::OpenBack && target.contains(20.0, 250.0)));
    assert!(targets
        .iter()
        .any(|target| target.action == GuiAction::SaveForm && target.contains(20.0, 282.0)));
    assert!(targets.iter().any(|target| {
        target.action == GuiAction::ToggleLinearView && target.contains(20.0, 314.0)
    }));
    assert!(targets.iter().any(|target| {
        matches!(&target.action, GuiAction::PlacePaletteKind(kind) if kind == "text/upper")
    }));
    assert!(targets
        .iter()
        .any(|target| matches!(&target.action, GuiAction::ConfigureGear { .. })));
    assert_eq!(
        targets
            .iter()
            .filter(|target| matches!(target.action, GuiAction::FlipGear(_)))
            .count(),
        graph.gears.len()
    );
}

#[test]
fn reverse_face_is_renderer_local_and_keeps_the_demo_graph_intact() {
    let graph = graph();
    let gear = &graph.gears[0];
    let subject = graph.subject_ref(&gear.identity).unwrap();
    let identities = (
        graph.source_document_id.clone(),
        graph.checked_form_id.clone(),
        graph.expanded_form_id.clone(),
        gear.identity.clone(),
    );
    let mut layout = patchbay_model::PatchbayLayout::default();
    layout.move_gear(&graph, &subject, 310, 140).unwrap();

    assert!(layout.flip_gear(&graph, &subject).unwrap());
    assert_eq!(layout.position(&gear.identity), Some((310, 140)));
    assert_eq!(
        identities,
        (
            graph.source_document_id.clone(),
            graph.checked_form_id.clone(),
            graph.expanded_form_id.clone(),
            graph.gears[0].identity.clone(),
        )
    );

    let mut pixels = vec![BACKGROUND; 1100 * 720];
    let targets = draw_patchbay(
        &mut pixels,
        1100,
        720,
        &graph,
        PatchbayViewContext {
            selected: Some(&gear.identity),
            breadcrumb: "",
            lifecycle: &LifecycleContext::default(),
            palette_query: "",
            presentation_layout: &layout,
            realization_plan: None,
            realization_hosts: &[],
        },
    );
    assert!(targets.iter().any(
        |target| matches!(&target.action, GuiAction::FlipGear(candidate) if candidate == &subject)
    ));
    assert!(!layout.flip_gear(&graph, &subject).unwrap());
    assert_eq!(layout.position(&gear.identity), Some((310, 140)));
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
        PatchbayViewContext {
            selected: Some(&selected),
            breadcrumb: "",
            lifecycle: &LifecycleContext::default(),
            palette_query: "",
            presentation_layout: &Default::default(),
            realization_plan: None,
            realization_hosts: &[],
        },
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

#[test]
fn palette_query_visibly_filters_the_authoritative_entries() {
    let graph = graph();
    let mut pixels = vec![BACKGROUND; 1100 * 720];
    let targets = draw_patchbay(
        &mut pixels,
        1100,
        720,
        &graph,
        PatchbayViewContext {
            selected: None,
            breadcrumb: "",
            lifecycle: &LifecycleContext::default(),
            palette_query: "value/count",
            presentation_layout: &Default::default(),
            realization_plan: None,
            realization_hosts: &[],
        },
    );
    let kinds = targets
        .iter()
        .filter_map(|target| match &target.action {
            GuiAction::PlacePaletteKind(kind) => Some(kind.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(kinds, ["state/count", "presentation/count"]);
}

#[test]
fn presentation_layout_moves_a_gear_without_changing_graph_or_cord_identity() {
    let graph = graph();
    let identities = (
        graph.source_document_id.clone(),
        graph.checked_form_id.clone(),
        graph.expanded_form_id.clone(),
        graph.cords.clone(),
    );
    let subject = graph.subject_ref(&graph.gears[0].identity).unwrap();
    let mut layout = patchbay_model::PatchbayLayout::default();
    layout.move_gear(&graph, &subject, 500, 300).unwrap();
    let mut pixels = vec![BACKGROUND; 1100 * 720];
    let targets = draw_patchbay(
        &mut pixels,
        1100,
        720,
        &graph,
        PatchbayViewContext {
            selected: None,
            breadcrumb: "",
            lifecycle: &LifecycleContext::default(),
            palette_query: "",
            presentation_layout: &layout,
            realization_plan: None,
            realization_hosts: &[],
        },
    );
    assert!(targets.iter().any(|target| {
        matches!(
            &target.action,
            GuiAction::SelectSubject(candidate) if candidate == &subject
        ) && target.contains(510.0, 310.0)
    }));
    assert_eq!(
        identities,
        (
            graph.source_document_id,
            graph.checked_form_id,
            graph.expanded_form_id,
            graph.cords,
        )
    );
}
