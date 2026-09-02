use crate::{
    gui::{
        draw_patchbay, draw_patchbay_with_debugger, GuiAction, LifecycleContext,
        PatchbayViewContext,
    },
    icon::Icon,
    render::BACKGROUND,
};
use patchbay_model::{FormEditor, PatchbayGraph, PHOSPHOR_THEME};
use std::path::PathBuf;

fn graph() -> PatchbayGraph {
    let editor = FormEditor::from_source(
        PathBuf::from("count.conduit"),
        include_str!("../../../../examples/count.conduit").into(),
    )
    .unwrap();
    PatchbayGraph::from_expanded(&editor.expand_form("count-demo").unwrap()).unwrap()
}

fn composition_graph() -> PatchbayGraph {
    FormEditor::from_source(
        PathBuf::from("greet.conduit"),
        include_str!("../../../../examples/greet.conduit").into(),
    )
    .unwrap()
    .patchbay_graph_for_authoring("default-welcome")
    .unwrap()
}

#[test]
fn native_renderer_consumes_exact_shared_debugger_activity_without_topology_mutation() {
    let graph = graph();
    let subject = graph.gears[0].identity.clone();
    let debugger: patchbay_model::DebuggerPresentation =
        serde_json::from_value(serde_json::json!({
            "schema": patchbay_model::DEBUGGER_PRESENTATION_SCHEMA,
            "execution": { "body": vec![1; 32], "plan": vec![2; 32], "play": vec![3; 32] },
            "revision": 1,
            "tick": 0,
            "reduced_motion": true,
            "gap": null,
            "activities": [{
                "subject": subject,
                "line_subject": null,
                "host": 7,
                "phase": "active",
                "latest_kind": "gear-started",
                "latest_sequence": 1,
                "observed_count": 1,
                "coalesced_count": 0,
                "last_activity_tick": 0,
                "latest_value": null,
                "retained_fault_code": null
            }]
        }))
        .unwrap();
    let before = graph.clone();
    let mut pixels = vec![BACKGROUND; 1100 * 720];
    draw_patchbay_with_debugger(
        &mut pixels,
        1100,
        720,
        &graph,
        PatchbayViewContext {
            selected: None,
            breadcrumb: "",
            lifecycle: &LifecycleContext::default(),
            palette: &Default::default(),
            forms: &[],
            form_selection: 0,
            form_scroll: 0,
            exact_identity_open: false,
            face_control_focus: 0,
            presentation_layout: &Default::default(),
            realization_plan: None,
            realization_hosts: &[],
            status: None,
            gesture: Default::default(),
            viewport: &Default::default(),
        },
        Some(&debugger),
    );
    assert_eq!(graph, before);
    assert!(pixels.contains(&PHOSPHOR_THEME.focus.packed_rgb()));
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
    let navigator = vec![crate::forms_navigation::FormNavigatorEntry {
        label: "ROOT welcome [ANCESTOR]".into(),
        action: Some(GuiAction::OpenNavigatorAncestor {
            source_document_id: graph.source_document_id.as_str().into(),
            checked_form_id: graph.checked_form_id.as_str().into(),
            expanded_form_id: graph.expanded_form_id.as_str().into(),
            back_count: 1,
        }),
    }];
    let visible_gears = graph
        .gears
        .iter()
        .filter(|gear| graph.compositions.is_empty() || gear.source_form == graph.form_name)
        .collect::<Vec<_>>();
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
            palette: &Default::default(),
            forms: &navigator,
            form_selection: 0,
            form_scroll: 0,
            exact_identity_open: false,
            face_control_focus: 0,
            presentation_layout: &Default::default(),
            realization_plan: None,
            realization_hosts: &[],
            status: None,
            gesture: Default::default(),
            viewport: &Default::default(),
        },
    );
    assert!(!visible_gears.is_empty());
    assert!(targets.len() <= crate::gui::MAX_HIT_TARGETS);
    assert!(pixels.contains(&PHOSPHOR_THEME.structure_primary.packed_rgb()));
    assert!(pixels.contains(&PHOSPHOR_THEME.emphasis.packed_rgb()));
    assert!(targets
        .iter()
        .any(|target| matches!(&target.action, GuiAction::SelectSubject(subject) if subject.subject_identity.starts_with("cord/") && subject.expanded_form_id == graph.expanded_form_id)));
    assert!(targets
        .iter()
        .any(|target| matches!(&target.action, GuiAction::SelectSubject(subject) if subject.subject_identity.starts_with("port/") && subject.expanded_form_id == graph.expanded_form_id)));
    assert!(targets.iter().any(|target| {
        matches!(target.action, GuiAction::OpenNavigatorAncestor { .. })
            && target.contains(20.0, 90.0)
    }));
    assert!(targets
        .iter()
        .any(|target| target.action == GuiAction::OpenBack && target.contains(20.0, 250.0)));
    assert!(targets
        .iter()
        .any(|target| target.action == GuiAction::SaveForm && target.contains(20.0, 282.0)));
    assert!(targets.iter().any(|target| {
        target.action == GuiAction::UndoSemanticEdit && target.contains(20.0, 314.0)
    }));
    assert!(targets.iter().any(|target| {
        target.action == GuiAction::RedoSemanticEdit && target.contains(20.0, 346.0)
    }));
    assert!(targets.iter().any(|target| {
        target.action == GuiAction::ToggleLinearView && target.contains(20.0, 378.0)
    }));
    assert!(targets
        .iter()
        .any(|target| { matches!(&target.action, GuiAction::BeginPaletteDrag(_)) }));
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
fn parent_canvas_draws_one_composed_gear_instead_of_its_expanded_child_gears() {
    let graph = composition_graph();
    assert_eq!(graph.compositions.len(), 1);
    assert!(graph.gears.iter().any(|gear| gear.source_form == "greet"));
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
            palette: &Default::default(),
            forms: &[],
            form_selection: 0,
            form_scroll: 0,
            exact_identity_open: false,
            face_control_focus: 0,
            presentation_layout: &Default::default(),
            realization_plan: None,
            realization_hosts: &[],
            status: None,
            gesture: Default::default(),
            viewport: &Default::default(),
        },
    );
    assert!(targets.iter().any(|target| {
        matches!(
            &target.action,
            GuiAction::SelectSubject(subject)
                if subject.subject_identity == "composition/hello"
        )
    }));
    assert!(!targets.iter().any(|target| {
        matches!(
            &target.action,
            GuiAction::SelectSubject(subject)
                if subject.subject_identity == "gear/default-welcome/hello/join"
        )
    }));
}

#[test]
fn every_patchbay_drag_state_has_a_distinct_visible_manifestation() {
    use embedded_graphics::prelude::Point;

    let graph = graph();
    let render = |gesture| {
        let mut pixels = vec![BACKGROUND; 1100 * 720];
        draw_patchbay(
            &mut pixels,
            1100,
            720,
            &graph,
            PatchbayViewContext {
                selected: None,
                breadcrumb: "",
                lifecycle: &LifecycleContext::default(),
                palette: &Default::default(),
                forms: &[],
                form_selection: 0,
                form_scroll: 0,
                exact_identity_open: false,
                face_control_focus: 0,
                presentation_layout: &Default::default(),
                realization_plan: None,
                realization_hosts: &[],
                status: None,
                gesture,
                viewport: &Default::default(),
            },
        );
        pixels
    };
    let baseline = render(Default::default());
    let cursor = Point::new(610, 420);
    let gestures = [
        crate::gui_gesture::GestureView {
            palette_kind: Some("text/upper"),
            cursor,
            ..Default::default()
        },
        crate::gui_gesture::GestureView {
            gear: Some(&graph.gears[0].identity),
            cursor,
            ..Default::default()
        },
        crate::gui_gesture::GestureView {
            cord_source: Some(&graph.cords[0].source_port),
            cursor,
            ..Default::default()
        },
        crate::gui_gesture::GestureView {
            cord_route: Some(&graph.cords[0].identity),
            cursor,
            ..Default::default()
        },
    ];
    for gesture in gestures {
        assert_ne!(render(gesture), baseline);
    }
}

#[test]
fn contextual_lifecycle_header_exposes_only_projected_typed_actions() {
    use crate::lifecycle_flow::{LifecycleFlow, LifecycleFlowAction};

    let graph = graph();
    let lifecycle = LifecycleContext {
        body_id: Some("body/exact".into()),
        wake_id: Some("wake/exact".into()),
        plan_id: Some("plan/exact".into()),
        play_id: None,
        body_workbench_destination: None,
        flow: LifecycleFlow {
            state_code: "PLAN_READY",
            state_text: "PLAN ready".into(),
            detail: "Exact Plan admitted; no Play active".into(),
            exact_basis: "body=body/exact wake=wake/exact plan=plan/exact play=none".into(),
            actions: vec![
                LifecycleFlowAction {
                    action: patchbay_model::PatchbayAction::Play,
                    label: "PLAY",
                    accelerator: "F7",
                },
                LifecycleFlowAction {
                    action: patchbay_model::PatchbayAction::Lull,
                    label: "LULL",
                    accelerator: "Shift+F6",
                },
            ],
        },
        parts: None,
        selected_part: None,
        selected_candidate: None,
        pending_revoke: None,
        browser_spawn_pending: false,
    };
    let mut pixels = vec![BACKGROUND; 1100 * 720];
    let targets = draw_patchbay(
        &mut pixels,
        1100,
        720,
        &graph,
        PatchbayViewContext {
            selected: None,
            breadcrumb: "",
            lifecycle: &lifecycle,
            palette: &Default::default(),
            forms: &[],
            form_selection: 0,
            form_scroll: 0,
            exact_identity_open: false,
            face_control_focus: 0,
            presentation_layout: &Default::default(),
            realization_plan: None,
            realization_hosts: &[],
            status: None,
            gesture: Default::default(),
            viewport: &Default::default(),
        },
    );
    let lifecycle_actions = targets
        .iter()
        .filter_map(|target| match &target.action {
            GuiAction::Lifecycle(action) => Some(*action),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        lifecycle_actions,
        [
            patchbay_model::PatchbayAction::Play,
            patchbay_model::PatchbayAction::Lull
        ]
    );
    assert!(pixels.contains(&patchbay_model::PHOSPHOR_THEME.focus.packed_rgb()));
}

#[test]
fn selected_inspector_has_one_visible_pointer_and_keyboard_exact_disclosure() {
    let graph = graph();
    let selected = graph.gears[0].identity.as_str();
    let mut pixels = vec![BACKGROUND; 1_100 * 720];
    let targets = draw_patchbay(
        &mut pixels,
        1_100,
        720,
        &graph,
        PatchbayViewContext {
            selected: Some(selected),
            breadcrumb: "",
            lifecycle: &Default::default(),
            palette: &Default::default(),
            forms: &[],
            form_selection: 0,
            form_scroll: 0,
            exact_identity_open: false,
            face_control_focus: 0,
            presentation_layout: &Default::default(),
            realization_plan: None,
            realization_hosts: &[],
            status: None,
            gesture: Default::default(),
            viewport: &Default::default(),
        },
    );
    let disclosure = targets
        .iter()
        .filter(|target| target.action == GuiAction::ToggleExactIdentity)
        .collect::<Vec<_>>();
    assert_eq!(disclosure.len(), 1);
    assert!(disclosure[0].contains(840.0, 266.0));

    let mut quiet_pixels = vec![BACKGROUND; 1_100 * 720];
    let quiet = draw_patchbay(
        &mut quiet_pixels,
        1_100,
        720,
        &graph,
        PatchbayViewContext {
            selected: None,
            breadcrumb: "",
            lifecycle: &Default::default(),
            palette: &Default::default(),
            forms: &[],
            form_selection: 0,
            form_scroll: 0,
            exact_identity_open: false,
            face_control_focus: 0,
            presentation_layout: &Default::default(),
            realization_plan: None,
            realization_hosts: &[],
            status: None,
            gesture: Default::default(),
            viewport: &Default::default(),
        },
    );
    assert!(!quiet
        .iter()
        .any(|target| target.action == GuiAction::ToggleExactIdentity));
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
            palette: &Default::default(),
            forms: &[],
            form_selection: 0,
            form_scroll: 0,
            exact_identity_open: false,
            face_control_focus: 0,
            presentation_layout: &layout,
            realization_plan: None,
            realization_hosts: &[],
            status: None,
            gesture: Default::default(),
            viewport: &Default::default(),
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
            palette: &Default::default(),
            forms: &[],
            form_selection: 0,
            form_scroll: 0,
            exact_identity_open: false,
            face_control_focus: 0,
            presentation_layout: &Default::default(),
            realization_plan: None,
            realization_hosts: &[],
            status: None,
            gesture: Default::default(),
            viewport: &Default::default(),
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
            palette: &crate::palette_state::PaletteChooser::for_query("value/count"),
            forms: &[],
            form_selection: 0,
            form_scroll: 0,
            exact_identity_open: false,
            face_control_focus: 0,
            presentation_layout: &Default::default(),
            realization_plan: None,
            realization_hosts: &[],
            status: None,
            gesture: Default::default(),
            viewport: &Default::default(),
        },
    );
    let kinds = targets
        .iter()
        .filter_map(|target| match &target.action {
            GuiAction::BeginPaletteDrag(kind) => Some(kind.as_str()),
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
            palette: &Default::default(),
            forms: &[],
            form_selection: 0,
            form_scroll: 0,
            exact_identity_open: false,
            face_control_focus: 0,
            presentation_layout: &layout,
            realization_plan: None,
            realization_hosts: &[],
            status: None,
            gesture: Default::default(),
            viewport: &Default::default(),
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
