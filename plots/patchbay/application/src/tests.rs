use super::*;
use conduit_plot::{ProfileCatalog, StartupCatalog};

fn plot() -> ExpandedCanonicalPlot {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_application_catalogs(&mut startup, &mut profile).unwrap();
    let syntax =
        conduit_plot::parse_syntax_document(include_str!("../../../../plots/tour/main.conduit"));
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    conduit_plot::expand_canonical_plot(&checked, "tour", &profile).unwrap()
}

#[test]
fn opens_the_exact_plot_and_plan_and_retains_inspection() {
    let plot = plot();
    let mut port = PatchbayApplicationPort::open(
        &plot,
        PlanId::from("plan/current"),
        PlanId::from("body-plan/current"),
    )
    .unwrap();
    let initial = port.apply(&[]).unwrap();
    let view = ApplicationView::decode(&initial.view).unwrap();
    let event = ApplicationEvent {
        revision: view.revision,
        action: INSPECT_NEXT_ACTION_ID.into(),
        kind: ApplicationEventKind::Activate,
        value: Vec::new(),
    };
    let next = port.apply(&event.encode(&view).unwrap()).unwrap();
    assert_eq!(ApplicationView::decode(&next.view).unwrap().revision, 2);
    assert_eq!(port.graph().expanded_plot_id, plot.expanded_plot_id);
    assert_eq!(port.plan_id().as_str(), "plan/current");
}

#[test]
fn active_body_opens_as_a_simple_plot_list_before_graph_inspection() {
    let plot = plot();
    let entries = vec![
        PatchbayActivePlot::project(
            &plot,
            "Friendly Tour",
            "checked/tour",
            PlanId::from("plan/tour"),
            PatchbayActivePlotState::Playing,
            false,
        )
        .unwrap(),
        PatchbayActivePlot::project(
            &plot,
            "Patchbay",
            "checked/patchbay",
            PlanId::from("plan/patchbay"),
            PatchbayActivePlotState::Playing,
            true,
        )
        .unwrap(),
    ];
    let mut port = PatchbayApplicationPort::open_active(
        entries,
        PlanId::from("body-plan/current"),
        Some(ActivePlayId::from("play/current")),
    )
    .unwrap();
    let initial = ApplicationView::decode(&port.apply(&[]).unwrap().view).unwrap();
    assert!(initial
        .nodes
        .iter()
        .any(|node| node.key == "active-plot-0" && node.text == "Friendly Tour"));
    assert!(initial
        .nodes
        .iter()
        .any(|node| node.key == "active-plot-status-1" && node.text == "Playing"));
    let disclosure = initial
        .nodes
        .iter()
        .position(|node| {
            node.component == ApplicationComponent::Disclosure && node.key == "exact-evidence"
        })
        .unwrap() as u8;
    assert!(initial.nodes.iter().any(|node| node.key == "checked-plot-1"
        && node.text.contains("checked/patchbay")
        && node.parent == Some(disclosure)));
    assert!(initial
        .nodes
        .iter()
        .any(|node| node.key == "body-play" && node.parent == Some(disclosure)));
    assert!(initial.nodes.iter().any(|node| node.key == "checked-plot-1"
        && node
            .text
            .contains("focused when this inspector was prepared")
        && node.parent == Some(disclosure)));
    assert!(!initial
        .nodes
        .iter()
        .any(|node| node.text.contains("foreground")));
    assert!(!initial.nodes.iter().any(|node| node.key == "subject"));
    assert!(initial
        .nodes
        .iter()
        .any(|node| node.key == "body-play" && node.text.contains("Play play/current")));

    let select = &initial.actions[0];
    let selected = port
        .apply(
            &ApplicationEvent {
                revision: initial.revision,
                action: select.id.clone(),
                kind: select.event,
                value: Vec::new(),
            }
            .encode(&initial)
            .unwrap(),
        )
        .unwrap();
    let selected = ApplicationView::decode(&selected.view).unwrap();
    assert!(selected.nodes.iter().any(|node| node.key == "subject"));
    assert!(selected
        .nodes
        .iter()
        .any(|node| node.key == "identity" && node.text.contains("Plan plan/tour")));
}

#[test]
fn stale_events_and_edit_authority_remain_explicit() {
    let plot = plot();
    let mut port = PatchbayApplicationPort::open(
        &plot,
        PlanId::from("plan/current"),
        PlanId::from("body-plan/current"),
    )
    .unwrap();
    let view = ApplicationView::decode(&port.apply(&[]).unwrap().view).unwrap();
    let edit = ApplicationEvent {
        revision: view.revision,
        action: EDIT_CURRENT_ACTION_ID.into(),
        kind: ApplicationEventKind::Activate,
        value: Vec::new(),
    };
    assert!(matches!(
        port.apply(&edit.encode(&view).unwrap()).unwrap().request,
        Some(PatchbayApplicationRequest::EditCurrent { .. })
    ));
    let stale = ApplicationEvent {
        revision: view.revision,
        action: INSPECT_NEXT_ACTION_ID.into(),
        kind: ApplicationEventKind::Activate,
        value: Vec::new(),
    };
    let stale_view = ApplicationView {
        revision: stale.revision,
        ..view
    };
    assert_eq!(
        port.apply(&stale.encode(&stale_view).unwrap()),
        Err(PatchbayApplicationRefusal::Event(
            ApplicationViewRefusal::StaleRevision
        ))
    );
}

#[test]
fn mask_topology_is_visible_and_requests_the_exact_body_plan() {
    let plot = plot();
    let mut port = PatchbayApplicationPort::open(
        &plot,
        PlanId::from("plan/current"),
        PlanId::from("body-plan/current"),
    )
    .unwrap();
    port.set_mask_topology(PatchbayMaskTopology {
        presentation_id: "presentation/current".into(),
        body_plan_id: PlanId::from("body-plan/current"),
        active_play_id: "play/current".into(),
        mode: PatchbayMaskMode::Graphical,
        stages: vec![PatchbayMaskStage {
            manifestation_id: "manifestation/graphical".into(),
            implementation_id: "presentation/renderer-conduitos-native@1".into(),
            host_id: "host/current".into(),
            boot_id: "boot/current".into(),
            resource_pool_id: "pool/mask".into(),
            resource_class_id: "resource/mask".into(),
            reserved_units: 1,
            maximum_active_instances: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 4096,
            available: true,
        }],
    });
    let view = ApplicationView::decode(&port.apply(&[]).unwrap().view).unwrap();
    assert_eq!(view.actions[0].id, "patchbay.plot.0");
    assert_eq!(view.actions[1].id, INSPECT_NEXT_ACTION_ID);
    assert_eq!(view.actions[2].id, EDIT_CURRENT_ACTION_ID);
    assert_eq!(view.actions[3].id, CHANGE_MASKS_ACTION_ID);
    assert!(view
        .nodes
        .iter()
        .any(|node| node.key == "mask-stage-0" && node.text.contains("available")));
    let action = view
        .actions
        .iter()
        .find(|action| action.id == CHANGE_MASKS_ACTION_ID)
        .unwrap();
    let changed = port
        .apply(
            &ApplicationEvent {
                revision: view.revision,
                action: action.id.clone(),
                kind: action.event,
                value: Vec::new(),
            }
            .encode(&view)
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        changed.request,
        Some(PatchbayApplicationRequest::ChangeMasks {
            body_plan_id: PlanId::from("body-plan/current"),
            mode: PatchbayMaskMode::GraphicalAndSpeech
        })
    );
}

#[test]
fn resident_canvas_preserves_exact_graph_and_authoritative_subject_selection() {
    use conduit_presentation::application_canvas::ApplicationCanvas;
    let plot = plot();
    let mut port = PatchbayApplicationPort::open(
        &plot,
        PlanId::from("plan/current"),
        PlanId::from("body-plan/current"),
    )
    .unwrap();
    let initial = port.apply(&[]).unwrap();
    let view = ApplicationView::decode(&initial.view).unwrap();
    let canvas = view
        .nodes
        .iter()
        .find(|node| node.component == ApplicationComponent::PatchbayCanvas)
        .unwrap();
    let graph = ApplicationCanvas::decode(&canvas.value).unwrap();
    assert_eq!(graph.checked_plot, plot.checked_plot_id.as_str());
    assert_eq!(graph.expanded_plot, plot.expanded_plot_id.as_str());
    assert_eq!(graph.plan, "plan/current");
    assert_eq!(graph.body_plan, "body-plan/current");
    assert_eq!(graph.cords.len(), port.graph().cords.len());
    let subject = String::from(port.graph().subject_identities().last().unwrap());
    let event = ApplicationEvent {
        revision: view.revision,
        action: resident_canvas::SELECT_SUBJECT.into(),
        kind: ApplicationEventKind::Change,
        value: subject.as_bytes().to_vec(),
    };
    let encoded = event.encode(&view).unwrap();
    port.apply(&encoded).unwrap();
    assert_eq!(port.inspection().unwrap().subject_identity, subject);
    assert!(
        port.apply(&encoded).is_err(),
        "stale graph interaction must refuse"
    );
    let current = ApplicationView::decode(&port.apply(&[]).unwrap().view).unwrap();
    let invalid = ApplicationEvent {
        revision: current.revision,
        value: b"not-a-resident-subject".to_vec(),
        ..event
    };
    assert!(port.apply(&invalid.encode(&current).unwrap()).is_err());
    assert_eq!(port.inspection().unwrap().subject_identity, subject);
}
