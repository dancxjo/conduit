use conduit_body::{Body, BodyFulfillment, FulfillmentObligation, Wake};
use conduit_core::{
    bind_active_play, seal_plan, AuthorityGrantId, CheckedFormId, ExpandedFormId, FormIdentity,
    SignId, SourceDocumentId,
};
use conduit_presentation::{
    ApplicationAction, ApplicationComponent, ApplicationEventKind, ApplicationNodeState,
    ApplicationView, ApplicationViewNode, Face, FaceContext, FaceContribution,
    FaceContributionRole, FaceFocus, FaceOperatorActionKind, FaceRefusal,
    PresentationPropertyValue, PresentationRole,
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

fn playing() -> (Body, Wake, conduit_core::PlanId, conduit_core::ActivePlayId) {
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
    (body, wake, plan.plan_id, play_id)
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
    let surface = Face::project(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
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
    assert!(surface.operator_actions.iter().any(|action| {
        action.kind == FaceOperatorActionKind::Wake
            && surface
                .resolve_operator_action(7, &action.surface_action_id)
                .is_ok()
    }));
}

#[test]
fn resident_view_joins_body_truth_only_for_its_current_play() {
    let (body, wake, plan_id, play_id) = playing();
    let contribution = FaceContribution {
        role: FaceContributionRole::Tutorial,
        checked_form_id: CheckedFormId::from("checked/tutorial"),
        plan_id: plan_id.clone(),
        active_play_id: play_id.clone(),
        view: tutorial_view(11),
    };
    let surface = Face::project(
        &body,
        Some(&wake),
        20,
        FaceContext::Tutorial(CheckedFormId::from("checked/tutorial")),
        FaceFocus::Contribution {
            role: FaceContributionRole::Tutorial,
            node_key: Some("continue".into()),
        },
        vec![contribution.clone()],
    )
    .unwrap();

    assert!(surface.presentation.properties.iter().any(|property| {
        property.name == "active-play-id"
            && property.value == PresentationPropertyValue::Identity(play_id.as_str().into())
    }));
    assert!(surface.presentation.properties.iter().any(|property| {
        property.name == "plan-id"
            && property.value == PresentationPropertyValue::Identity(plan_id.as_str().into())
    }));
    assert!(surface
        .presentation
        .subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Plan));
    assert!(surface
        .presentation
        .subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Play));
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
    assert_eq!(routed.plan_id, plan_id);
    assert_eq!(routed.application_view_revision, 11);
    assert_eq!(routed.application_action_id, "tutorial.continue");
    assert_eq!(
        surface.resolve_application_action(19, &action.identity),
        Err(FaceRefusal::StaleAction)
    );
    let library = surface
        .operator_actions
        .iter()
        .find(|action| action.kind == FaceOperatorActionKind::OpenLibrary)
        .unwrap();
    assert!(surface
        .resolve_operator_action(20, &library.surface_action_id)
        .is_ok());
    assert_eq!(
        surface.resolve_operator_action(19, &library.surface_action_id),
        Err(FaceRefusal::StaleAction)
    );
    let inspection = surface
        .operator_actions
        .iter()
        .find(|action| {
            action.kind
                == FaceOperatorActionKind::OpenInspection(CheckedFormId::from("checked/tutorial"))
        })
        .unwrap();
    assert_eq!(
        surface.resolve_operator_action(20, &inspection.surface_action_id),
        Err(FaceRefusal::UnavailableAction)
    );

    let lulled = born_body();
    assert_eq!(
        Face::project(
            &lulled,
            None,
            21,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![contribution],
        ),
        Err(FaceRefusal::PlayNotCurrent)
    );
}

#[test]
fn presentation_only_navigation_does_not_change_body_or_running_work() {
    let (body, wake, plan_id, play_id) = playing();
    let contribution = FaceContribution {
        role: FaceContributionRole::Foreground,
        checked_form_id: CheckedFormId::from("checked/tutorial"),
        plan_id,
        active_play_id: play_id,
        view: tutorial_view(4),
    };
    let overview = Face::project(
        &body,
        Some(&wake),
        1,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![contribution.clone()],
    )
    .unwrap();
    let foreground = Face::project(
        &body,
        Some(&wake),
        2,
        FaceContext::ResidentForm(CheckedFormId::from("checked/tutorial")),
        FaceFocus::Contribution {
            role: FaceContributionRole::Foreground,
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
    let (body, wake, plan_id, play_id) = playing();
    let contribution = FaceContribution {
        role: FaceContributionRole::Tutorial,
        checked_form_id: CheckedFormId::from("checked/tutorial"),
        plan_id,
        active_play_id: play_id,
        view: tutorial_view(8),
    };
    let project = || {
        Face::project(
            &body,
            Some(&wake),
            31,
            FaceContext::Tutorial(CheckedFormId::from("checked/tutorial")),
            FaceFocus::Body,
            vec![contribution.clone()],
        )
        .unwrap()
    };
    let first = project();
    let repeat = project();
    assert_eq!(first.presentation.identity, repeat.presentation.identity);
    assert_eq!(
        Face::project(
            &body,
            Some(&wake),
            32,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![contribution.clone(), contribution],
        ),
        Err(FaceRefusal::DuplicateRole)
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
    let surface = Face::project(
        &fulfilled,
        None,
        40,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap();

    assert!(surface.application_actions.is_empty());
    assert!(!surface.operator_actions.iter().any(|action| matches!(
        action.kind,
        FaceOperatorActionKind::Wake | FaceOperatorActionKind::Lull
    )));
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
