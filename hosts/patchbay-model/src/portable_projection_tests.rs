use conduit_body::Body;
use conduit_core::{bind_active_play, ConnectionBase, SignId};
use conduit_presentation::{
    PresentationPropertyValue, PresentationRole, MAX_PRESENTATION_TOTAL_BYTES,
};
use conduit_std_host::{StdHost, ThreadTimer};

use crate::{
    DistributedRouteDemo, FormEditor, PatchbayGraph, PatchbayPresentation, PatchbayRequestId,
    PlanDocument, PlayDocument, PortableProjectionError,
};

fn living_portable() -> (
    PatchbayPresentation,
    Body,
    conduit_body::Wake,
    conduit_presentation::Presentation,
) {
    let editor = FormEditor::from_source(
        "hello.conduit".into(),
        include_str!("../../../examples/hello.conduit").into(),
    )
    .unwrap();
    let expanded = editor.expand_form("hello").unwrap();
    let mut host = StdHost::new();
    let host_id = host.advertisement().host_id.clone();
    let boot_id = host.advertisement().boot_id.clone();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let plan_document =
        PlanDocument::from_plan(PatchbayRequestId::new("portable/plan").unwrap(), &plan).unwrap();
    let mut output = Vec::with_capacity(4096);
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut ThreadTimer)
        .unwrap();
    let play_document = PlayDocument::from_report(&plan, &report).unwrap();

    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_form_id.clone(),
        1,
        SignId::from("patchbay/bornd"),
    )
    .unwrap();
    let (body, wake) = body.wake(1, SignId::from("patchbay/woke")).unwrap();
    let wake = wake
        .plan_ready(&plan, SignId::from("patchbay/planned"))
        .unwrap();
    let active_play = bind_active_play(&plan.plan_id, &host_id, &boot_id, 0);
    assert_eq!(active_play.active_play_id, play_document.active_play_id);
    let wake = wake
        .play_started(&active_play, SignId::from("patchbay/playing"))
        .unwrap();
    let route = DistributedRouteDemo::build().unwrap();
    let graph = PatchbayGraph::from_expanded(&expanded).unwrap();
    let projection = PatchbayPresentation::new(
        7,
        editor.view(),
        Some(plan_document),
        Some(play_document),
        None,
        vec![route.presentation().clone()],
    )
    .unwrap()
    .with_graph(graph)
    .unwrap();
    let portable = projection.to_portable(&body, &wake).unwrap();
    (projection, body, wake, portable)
}

#[test]
fn living_patchbay_projection_preserves_lifecycle_plan_play_and_sign() {
    let (projection, body, wake, portable) = living_portable();
    let identities = projection.identities();
    assert_eq!(portable.basis.seed_id, body.seed_id);
    assert_eq!(portable.basis.body_id, body.body_id);
    assert_eq!(portable.basis.wake_id, wake.wake_id);
    assert_eq!(portable.basis.plan_id, identities.plan_id);
    assert_eq!(portable.basis.active_play_id, identities.active_play_id);
    for sign in identities
        .sign_ids
        .iter()
        .chain(body.sign_ids.iter())
        .chain(wake.sign_ids.iter())
    {
        assert!(portable.basis.sign_ids.contains(sign));
    }
    assert!(portable
        .subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Document));
    assert!(portable
        .subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Plan));
    assert!(portable
        .subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Play));
    assert_eq!(
        portable
            .subjects
            .iter()
            .filter(|subject| subject.role == PresentationRole::Gear)
            .count(),
        projection.graph.as_ref().unwrap().gears.len()
    );
    assert_eq!(
        portable
            .subjects
            .iter()
            .filter(|subject| subject.role == PresentationRole::Port)
            .count(),
        projection
            .graph
            .as_ref()
            .unwrap()
            .gears
            .iter()
            .map(|gear| gear.inputs.len() + gear.outputs.len())
            .sum::<usize>()
    );
    assert!(portable.properties.iter().any(|property| {
        property.name == "icon-token"
            && property.value == PresentationPropertyValue::Text("case-upper".into())
    }));
    assert!(portable.relationships.iter().any(|relationship| {
        relationship.kind == conduit_presentation::PresentationRelationshipKind::Connects
    }));
    for required in [
        "plan-id",
        "plan-status",
        "placement-id",
        "host-id",
        "boot-id",
        "implementation-id",
        "artifact-id",
        "admitted-capacity",
        "active-play-id",
        "play-state",
        "pressure",
    ] {
        assert!(portable
            .properties
            .iter()
            .any(|property| property.name == required));
    }
    assert!(portable.properties.iter().any(|property| {
        property.name == "plan-status"
            && property.value == PresentationPropertyValue::Text("active".into())
    }));
    assert!(portable.properties.iter().any(|property| {
        property.name == "line"
            && property.value
                == PresentationPropertyValue::Text("local Cord; no external Line".into())
    }));
    assert!(!portable.properties.iter().any(|property| {
        property.name == "plan-status"
            && property.value == PresentationPropertyValue::Text("candidate".into())
    }));
    assert!(portable.properties.iter().any(|property| {
        property.name == "base"
            && property.value == PresentationPropertyValue::ConnectionBase(ConnectionBase::UsbCdc)
    }));
    assert!(portable.properties.iter().any(|property| {
        property.name == "base"
            && property.value
                == PresentationPropertyValue::ConnectionBase(ConnectionBase::WebSocket)
    }));
    assert!(portable.validate().is_ok());
}

#[test]
fn renderer_local_state_cannot_change_portable_content_identity() {
    let (projection, body, wake, portable) = living_portable();
    let local_wayland_state = (120_i32, -45_i32, 175_u16);
    let local_dom_state = ("viewport-node-72", true, 44_u16);
    assert_ne!(
        format!("{local_wayland_state:?}"),
        format!("{local_dom_state:?}")
    );
    assert_eq!(
        projection.to_portable(&body, &wake).unwrap().identity,
        portable.identity
    );
}

#[test]
fn stale_or_terminal_lifecycle_cannot_masquerade_as_live_presentation() {
    let (projection, body, wake, _) = living_portable();
    let lulled = wake.lull(SignId::from("patchbay/lulled")).unwrap();
    assert_eq!(
        projection.to_portable(&body, &lulled),
        Err(PortableProjectionError::PlayMismatch)
    );

    let mut stale_body = body;
    stale_body.source_document_id = "other-source".into();
    assert!(matches!(
        projection.to_portable(&stale_body, &wake),
        Err(PortableProjectionError::InvalidBody(_))
    ));
}

#[test]
fn portable_projection_remains_inside_the_reviewed_aggregate_bound() {
    let (_, _, _, portable) = living_portable();
    let encoded = serde_json::to_vec(&portable).unwrap();
    assert!(encoded.len() <= MAX_PRESENTATION_TOTAL_BYTES);
}

#[test]
fn portable_graph_must_share_the_open_form_identity_chain() {
    let (projection, _, _, _) = living_portable();
    let mut graph = projection.graph.clone().unwrap();
    graph.expanded_form_id = "expanded/stale".into();
    let without_graph = PatchbayPresentation::new(
        projection.revision,
        projection.document,
        projection.plan,
        projection.play,
        projection.topology,
        projection.routes,
    )
    .unwrap();
    assert_eq!(
        without_graph.with_graph(graph),
        Err(crate::RendererProjectionError::GraphBasisMismatch)
    );
}
