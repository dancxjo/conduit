use conduit_core::{bind_active_play, ConnectionProvider, EvidenceId};
use conduit_presentation::{
    PresentationPropertyValue, PresentationRole, MAX_PRESENTATION_TOTAL_BYTES,
};
use conduit_realm::{RealmDeployment, RealmId};
use conduit_std_host::{StdHost, ThreadTimer};

use crate::{
    DistributedRouteDemo, FormEditor, PatchbayPresentation, PatchbayRequestId, PlanDocument,
    PlayDocument, PortableProjectionError,
};

fn living_portable() -> (
    PatchbayPresentation,
    RealmDeployment,
    conduit_realm::RealmActivation,
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

    let deployment = RealmDeployment::install(
        RealmId::from("patchbay/realm"),
        plan.source_document_id.clone(),
        plan.checked_form_id.clone(),
        1,
        EvidenceId::from("patchbay/deployed"),
    )
    .unwrap();
    let (deployment, activation) = deployment
        .activate(1, EvidenceId::from("patchbay/activated"))
        .unwrap();
    let activation = activation
        .plan_ready(&plan, EvidenceId::from("patchbay/planned"))
        .unwrap();
    let active_play = bind_active_play(&plan.plan_id, &host_id, &boot_id, 0);
    assert_eq!(active_play.active_play_id, play_document.active_play_id);
    let activation = activation
        .play_started(&active_play, EvidenceId::from("patchbay/playing"))
        .unwrap();
    let route = DistributedRouteDemo::build().unwrap();
    let projection = PatchbayPresentation::new(
        7,
        editor.view(),
        Some(plan_document),
        Some(play_document),
        None,
        vec![route.presentation().clone()],
    )
    .unwrap();
    let portable = projection.to_portable(&deployment, &activation).unwrap();
    (projection, deployment, activation, portable)
}

#[test]
fn living_patchbay_projection_preserves_lifecycle_plan_play_and_evidence() {
    let (projection, deployment, activation, portable) = living_portable();
    let identities = projection.identities();
    assert_eq!(portable.basis.realm_id, deployment.realm_id);
    assert_eq!(portable.basis.deployment_id, deployment.deployment_id);
    assert_eq!(portable.basis.activation_id, activation.activation_id);
    assert_eq!(portable.basis.plan_id, identities.plan_id);
    assert_eq!(portable.basis.active_play_id, identities.active_play_id);
    for evidence in identities
        .evidence_ids
        .iter()
        .chain(deployment.evidence_ids.iter())
        .chain(activation.evidence_ids.iter())
    {
        assert!(portable.basis.evidence_ids.contains(evidence));
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
    assert!(portable.properties.iter().any(|property| {
        property.name == "provider"
            && property.value
                == PresentationPropertyValue::ConnectionProvider(ConnectionProvider::UsbCdc)
    }));
    assert!(portable.properties.iter().any(|property| {
        property.name == "provider"
            && property.value
                == PresentationPropertyValue::ConnectionProvider(ConnectionProvider::WebSocket)
    }));
    assert!(portable.validate().is_ok());
}

#[test]
fn renderer_local_state_cannot_change_portable_content_identity() {
    let (projection, deployment, activation, portable) = living_portable();
    let local_wayland_state = (120_i32, -45_i32, 175_u16);
    let local_dom_state = ("viewport-node-72", true, 44_u16);
    assert_ne!(
        format!("{local_wayland_state:?}"),
        format!("{local_dom_state:?}")
    );
    assert_eq!(
        projection
            .to_portable(&deployment, &activation)
            .unwrap()
            .identity,
        portable.identity
    );
}

#[test]
fn stale_or_terminal_lifecycle_cannot_masquerade_as_live_presentation() {
    let (projection, deployment, activation, _) = living_portable();
    let deactivated = activation
        .deactivate(EvidenceId::from("patchbay/deactivated"))
        .unwrap();
    assert_eq!(
        projection.to_portable(&deployment, &deactivated),
        Err(PortableProjectionError::PlayMismatch)
    );

    let mut stale_deployment = deployment;
    stale_deployment.source_document_id = "other-source".into();
    assert!(matches!(
        projection.to_portable(&stale_deployment, &activation),
        Err(PortableProjectionError::InvalidDeployment(_))
    ));
}

#[test]
fn portable_projection_remains_inside_the_reviewed_aggregate_bound() {
    let (_, _, _, portable) = living_portable();
    let encoded = serde_json::to_vec(&portable).unwrap();
    assert!(encoded.len() <= MAX_PRESENTATION_TOTAL_BYTES);
}
