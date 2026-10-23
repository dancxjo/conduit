use super::*;

fn prepared() -> PatchbayApplicationPort {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_application_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!("../../../../plots/tour/main.conduit")),
        &startup,
    )
    .unwrap();
    let expanded = conduit_plot::expand_canonical_plot(&checked, "tour", &profile).unwrap();
    PatchbayApplicationPort::open(&expanded, "plan/real".into(), "body/real".into())
        .unwrap()
        .with_prepared_graph_capacity(2)
        .unwrap()
}
fn source_graph() -> PatchbayGraph {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_face::install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(include_str!("../../../../plots/vision/main.conduit")),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "vision", &profile).unwrap();
    PatchbayGraph::from_authoring(&expanded).unwrap()
}
fn append(port: &mut PatchbayApplicationPort) -> usize {
    port.append_prepared_source_graph(source_graph(), &"body/real".into())
        .unwrap()
}
#[test]
fn checked_source_selection_preserves_graph_custody_and_refuses_edit() {
    let mut port = prepared();
    let index = append(&mut port);
    let pointer = port.plots[index].graph.gears.as_ptr();
    let capacity = port.plots.capacity();
    let selected = port
        .apply_event(&ApplicationEvent {
            revision: 1,
            action: format!("{SELECT_PLOT_ACTION_PREFIX}{index}"),
            kind: ApplicationEventKind::Activate,
            value: Vec::new(),
        })
        .unwrap();
    assert!(selected.is_none());
    assert_eq!(port.selected_plan_id(), None);
    assert_eq!(port.plan_id().as_str(), "plan/real");
    assert_eq!(port.plots[index].graph.gears.as_ptr(), pointer);
    assert_eq!(port.plots.capacity(), capacity);
    assert!(!port.plots[index].focused);
    let view = port.view().unwrap();
    assert!(view
        .nodes
        .iter()
        .any(|node| node.text == "Checked Source · no Plan or Play"));
    assert!(!view
        .actions
        .iter()
        .any(|action| action.id == EDIT_CURRENT_ACTION_ID));
    assert!(!view
        .nodes
        .iter()
        .any(|node| node.component == ApplicationComponent::PatchbayCanvas));
    let revision = port.revision();
    assert_eq!(
        port.apply_event(&ApplicationEvent {
            revision,
            action: EDIT_CURRENT_ACTION_ID.into(),
            kind: ApplicationEventKind::Activate,
            value: Vec::new()
        }),
        Err(PatchbayApplicationRefusal::UnknownAction)
    );
    assert_eq!(port.revision(), revision);
    port.select_prepared_plot(0).unwrap();
    assert!(port
        .view()
        .unwrap()
        .actions
        .iter()
        .any(|action| action.id == EDIT_CURRENT_ACTION_ID));
}
#[test]
fn source_preparation_checks_capacity_body_and_duplicate_identity_before_mutation() {
    let mut port = prepared();
    let graph = source_graph();
    assert_eq!(
        port.append_prepared_source_graph(graph.clone(), &"body/other".into()),
        Err(PatchbayApplicationRefusal::UnknownAction)
    );
    assert_eq!(port.plots.len(), 1);
    append(&mut port);
    assert_eq!(
        port.append_prepared_source_graph(graph, &"body/real".into()),
        Err(PatchbayApplicationRefusal::TooManyActivePlots)
    );
    assert_eq!(port.plots.len(), 2);
    let mut port = port.with_prepared_graph_capacity(3).unwrap();
    assert_eq!(
        port.append_prepared_source_graph(source_graph(), &"body/real".into()),
        Err(PatchbayApplicationRefusal::UnknownAction)
    );
    assert_eq!(port.plots.len(), 2);
}

#[test]
fn source_fragment_declares_observation_and_absence_without_membership_or_edit() {
    use conduit_face::{FaceContributionBasis, FacePropertyValue, FaceRelationshipKind};
    let mut port = prepared();
    let graph = source_graph();
    let identity = format!("plot/{}", graph.checked_plot_id.as_str());
    let index = port
        .append_prepared_source_graph(graph, &"body/real".into())
        .unwrap();
    port.select_prepared_plot(index).unwrap();
    let body = serde_json::from_str("\"body/owner\"").unwrap();
    let fragment = port
        .face_fragment(
            FaceContributionBasis {
                checked_plot_id: "checked/owner".into(),
                plan_id: "plan/owner".into(),
                active_play_id: "play/owner".into(),
                required_interaction_context: None,
            },
            &body,
            true,
        )
        .unwrap();
    assert!(fragment
        .subjects
        .iter()
        .any(|subject| subject.identity == identity));
    assert!(!fragment
        .relationships
        .iter()
        .any(|r| r.target == identity && r.kind == FaceRelationshipKind::Contains));
    assert!(!fragment
        .properties
        .iter()
        .any(|p| p.subject == identity && p.name == "observed-plan-id"));
    for name in ["inspected-plan-present", "inspected-play-present"] {
        assert!(fragment.properties.iter().any(|p| p.subject == identity
            && p.name == name
            && p.value == FacePropertyValue::Flag(false)));
    }
    assert!(!fragment
        .actions
        .iter()
        .any(|action| action.identity == EDIT_CURRENT_ACTION_ID));
    assert_eq!(port.selected_graph().unwrap().cords.len(), 16);
}
