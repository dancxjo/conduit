use conduit_body::{Body, BodyFulfillment, FulfillmentObligation, Wake};
use conduit_core::{
    bind_active_play, seal_plan, AuthorityGrantId, CheckedFormId, ExpandedFormId, FormIdentity,
    SignId, SourceDocumentId,
};
use conduit_presentation::{
    ApplicationAction, ApplicationComponent, ApplicationEventKind, ApplicationNodeState,
    ApplicationView, ApplicationViewNode, BodySurface, BodySurfaceContext, BodySurfaceContribution,
    BodySurfaceContributionRole, BodySurfaceFocus, BodySurfaceRefusal, PresentationPropertyValue,
};

fn born_body() -> Body {
    Body::born(
        SourceDocumentId::from("source/tutorial"),
        CheckedFormId::from("checked/tutorial"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
}

fn playing() -> (Body, Wake, conduit_core::ActivePlayId) {
    let (body, wake) = born_body().wake(2, SignId::from("sign/woke")).unwrap();
    let plan = seal_plan(
        FormIdentity {
            source_document_id: SourceDocumentId::from("source/tutorial"),
            checked_form_id: CheckedFormId::from("checked/tutorial"),
            expanded_form_id: ExpandedFormId::from("expanded/tutorial"),
        },
        vec![],
    );
    let wake = wake
        .plan_ready(&plan, SignId::from("sign/planned"))
        .unwrap();
    let play = bind_active_play(
        &plan.plan_id,
        &"host/tutorial".into(),
        &"boot/tutorial".into(),
        3,
    );
    let play_id = play.active_play_id.clone();
    let wake = wake
        .play_started(&play, SignId::from("sign/playing"))
        .unwrap();
    (body, wake, play_id)
}

fn tutorial_view(revision: u32) -> ApplicationView {
    ApplicationView {
        revision,
        nodes: vec![
            ApplicationViewNode {
                parent: None,
                component: ApplicationComponent::Main,
                key: "tutorial".into(),
                text: "Learn this Body".into(),
                value: String::new(),
                value_capacity: 0,
                action: None,
                state: ApplicationNodeState::Ready,
            },
            ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::Button,
                key: "continue".into(),
                text: "Continue".into(),
                value: String::new(),
                value_capacity: 0,
                action: Some(0),
                state: ApplicationNodeState::Ready,
            },
        ],
        actions: vec![ApplicationAction {
            id: "tutorial.continue".into(),
            event: ApplicationEventKind::Activate,
        }],
    }
}

#[test]
fn lulled_body_has_an_exact_surface_without_a_running_form() {
    let body = born_body();
    let surface = BodySurface::project(
        &body,
        None,
        7,
        BodySurfaceContext::Overview,
        BodySurfaceFocus::Body,
        vec![],
    )
    .unwrap();

    surface.presentation.validate().unwrap();
    assert_eq!(surface.presentation.basis.body_id, Some(body.body_id));
    assert_eq!(surface.presentation.basis.wake_id, None);
    assert_eq!(surface.presentation.revision, 7);
    assert!(surface.presentation.properties.iter().any(|property| {
        property.name == "lifecycle-state"
            && property.value == PresentationPropertyValue::Text("lulled".into())
    }));
    assert_eq!(surface.presentation.actions.len(), 1);
    assert_eq!(surface.presentation.actions[0].label, "Wake");
}

#[test]
fn resident_view_joins_body_truth_only_for_its_current_play() {
    let (body, wake, play_id) = playing();
    let contribution = BodySurfaceContribution {
        role: BodySurfaceContributionRole::Tutorial,
        checked_form_id: CheckedFormId::from("checked/tutorial"),
        active_play_id: play_id.clone(),
        view: tutorial_view(11),
    };
    let surface = BodySurface::project(
        &body,
        Some(&wake),
        20,
        BodySurfaceContext::Tutorial(CheckedFormId::from("checked/tutorial")),
        BodySurfaceFocus::Contribution {
            role: BodySurfaceContributionRole::Tutorial,
            node_key: Some("continue".into()),
        },
        vec![contribution.clone()],
    )
    .unwrap();

    assert!(surface.presentation.properties.iter().any(|property| {
        property.name == "active-play-id"
            && property.value == PresentationPropertyValue::Identity(play_id.as_str().into())
    }));
    let action = surface
        .presentation
        .actions
        .iter()
        .find(|action| action.label == "Continue")
        .unwrap();
    assert!(surface
        .presentation
        .resolve_action(20, &action.identity)
        .is_ok());
    let routed = surface
        .resolve_application_action(20, &action.identity)
        .unwrap();
    assert_eq!(routed.active_play_id, play_id);
    assert_eq!(routed.application_view_revision, 11);
    assert_eq!(routed.application_action_id, "tutorial.continue");
    assert_eq!(
        surface.resolve_application_action(19, &action.identity),
        Err(BodySurfaceRefusal::StaleAction)
    );

    let lulled = born_body();
    assert_eq!(
        BodySurface::project(
            &lulled,
            None,
            21,
            BodySurfaceContext::Overview,
            BodySurfaceFocus::Body,
            vec![contribution],
        ),
        Err(BodySurfaceRefusal::PlayNotCurrent)
    );
}

#[test]
fn presentation_only_navigation_does_not_change_body_or_running_work() {
    let (body, wake, play_id) = playing();
    let contribution = BodySurfaceContribution {
        role: BodySurfaceContributionRole::Foreground,
        checked_form_id: CheckedFormId::from("checked/tutorial"),
        active_play_id: play_id,
        view: tutorial_view(4),
    };
    let overview = BodySurface::project(
        &body,
        Some(&wake),
        1,
        BodySurfaceContext::Overview,
        BodySurfaceFocus::Body,
        vec![contribution.clone()],
    )
    .unwrap();
    let foreground = BodySurface::project(
        &body,
        Some(&wake),
        2,
        BodySurfaceContext::ResidentForm(CheckedFormId::from("checked/tutorial")),
        BodySurfaceFocus::Contribution {
            role: BodySurfaceContributionRole::Foreground,
            node_key: None,
        },
        vec![contribution],
    )
    .unwrap();

    assert_eq!(
        overview.presentation.basis.body_id,
        foreground.presentation.basis.body_id
    );
    assert_eq!(
        overview.presentation.basis.wake_id,
        foreground.presentation.basis.wake_id
    );
    assert_eq!(body.workload_revision, 0);
    assert_eq!(wake.plans.len(), 1);
    assert_ne!(
        overview.presentation.identity,
        foreground.presentation.identity
    );
}

#[test]
fn composition_is_finite_and_deterministic() {
    let (body, wake, play_id) = playing();
    let contribution = BodySurfaceContribution {
        role: BodySurfaceContributionRole::Tutorial,
        checked_form_id: CheckedFormId::from("checked/tutorial"),
        active_play_id: play_id,
        view: tutorial_view(8),
    };
    let project = || {
        BodySurface::project(
            &body,
            Some(&wake),
            31,
            BodySurfaceContext::Tutorial(CheckedFormId::from("checked/tutorial")),
            BodySurfaceFocus::Body,
            vec![contribution.clone()],
        )
        .unwrap()
    };
    let first = project();
    let repeat = project();
    assert_eq!(first.presentation.identity, repeat.presentation.identity);
    assert_eq!(
        BodySurface::project(
            &body,
            Some(&wake),
            32,
            BodySurfaceContext::Overview,
            BodySurfaceFocus::Body,
            vec![contribution.clone(), contribution],
        ),
        Err(BodySurfaceRefusal::DuplicateRole)
    );
}

#[test]
fn fulfilled_body_keeps_terminal_surface_without_wake_or_actions() {
    let fulfilled = born_body()
        .fulfill(
            BodyFulfillment {
                final_wake_id: None,
                authority_grant_id: AuthorityGrantId::from("grant/operator-fulfill"),
                attribution: "operator/alice".into(),
                settled_obligations: vec![FulfillmentObligation {
                    obligation_id: "obligation/closed".into(),
                    settlement_sign_id: SignId::from("sign/closed"),
                }],
            },
            SignId::from("sign/fulfilled"),
        )
        .unwrap();
    let surface = BodySurface::project(
        &fulfilled,
        None,
        40,
        BodySurfaceContext::Overview,
        BodySurfaceFocus::Body,
        vec![],
    )
    .unwrap();

    assert!(surface.presentation.actions.is_empty());
    assert!(surface.application_actions.is_empty());
    assert!(surface.presentation.properties.iter().any(|property| {
        property.name == "lifecycle-state"
            && property.value == PresentationPropertyValue::Text("fulfilled".into())
    }));
    assert!(surface
        .presentation
        .basis
        .sign_ids
        .iter()
        .any(|sign| sign.as_str() == "sign/fulfilled"));
}
