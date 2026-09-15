use super::*;
use conduit_form::{ProfileCatalog, StartupCatalog};

fn form() -> ExpandedCanonicalForm {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_application_catalogs(&mut startup, &mut profile).unwrap();
    let syntax =
        conduit_form::parse_syntax_document(include_str!("../../../../forms/tour/main.conduit"));
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    conduit_form::expand_canonical_form(&checked, "tour", &profile).unwrap()
}

#[test]
fn opens_the_exact_form_and_plan_and_retains_inspection() {
    let form = form();
    let mut port = PatchbayApplicationPort::open(
        &form,
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
    assert_eq!(port.graph().expanded_form_id, form.expanded_form_id);
    assert_eq!(port.plan_id().as_str(), "plan/current");
}

#[test]
fn active_body_opens_as_a_simple_form_list_before_graph_inspection() {
    let form = form();
    let entries = vec![
        PatchbayActiveForm::project(
            &form,
            "Friendly Tour",
            "checked/tour",
            PlanId::from("plan/tour"),
            PatchbayActiveFormState::Playing,
            false,
        )
        .unwrap(),
        PatchbayActiveForm::project(
            &form,
            "Patchbay",
            "checked/patchbay",
            PlanId::from("plan/patchbay"),
            PatchbayActiveFormState::Playing,
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
        .any(|node| node.key == "active-form-0" && node.text == "Friendly Tour"));
    assert!(initial
        .nodes
        .iter()
        .any(|node| node.key == "active-form-status-1"
            && node
                .text
                .contains("Playing · foreground · checked checked/patchbay")));
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
    let form = form();
    let mut port = PatchbayApplicationPort::open(
        &form,
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
fn presenter_topology_is_visible_and_requests_the_exact_body_plan() {
    let form = form();
    let mut port = PatchbayApplicationPort::open(
        &form,
        PlanId::from("plan/current"),
        PlanId::from("body-plan/current"),
    )
    .unwrap();
    port.set_presenter_topology(PatchbayPresenterTopology {
        presentation_id: "presentation/current".into(),
        body_plan_id: PlanId::from("body-plan/current"),
        active_play_id: "play/current".into(),
        mode: PatchbayPresenterMode::Graphical,
        stages: vec![PatchbayPresenterStage {
            manifestation_id: "manifestation/graphical".into(),
            implementation_id: "presentation/renderer-conduitos-native@1".into(),
            host_id: "host/current".into(),
            boot_id: "boot/current".into(),
            resource_pool_id: "pool/presenter".into(),
            resource_class_id: "resource/presenter".into(),
            reserved_units: 1,
            maximum_active_instances: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 4096,
            available: true,
        }],
    });
    let view = ApplicationView::decode(&port.apply(&[]).unwrap().view).unwrap();
    assert_eq!(view.actions[0].id, "patchbay.form.0");
    assert_eq!(view.actions[1].id, INSPECT_NEXT_ACTION_ID);
    assert_eq!(view.actions[2].id, EDIT_CURRENT_ACTION_ID);
    assert_eq!(view.actions[3].id, CHANGE_PRESENTERS_ACTION_ID);
    assert!(view
        .nodes
        .iter()
        .any(|node| node.key == "presenter-stage-0" && node.text.contains("available")));
    let action = view
        .actions
        .iter()
        .find(|action| action.id == CHANGE_PRESENTERS_ACTION_ID)
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
        Some(PatchbayApplicationRequest::ChangePresenters {
            body_plan_id: PlanId::from("body-plan/current"),
            mode: PatchbayPresenterMode::GraphicalAndSpeech
        })
    );
}
