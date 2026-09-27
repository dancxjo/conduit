use super::*;
use conduit_core::{CheckedFormId, ExpandedFormId, PlanId, SourceDocumentId};
use conduit_presentation::{
    PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationDisclosureLevel, PresentationInput, PresentationRole, PresentationSubject,
    PresentationText, UTF8_TEXT_VALUE_KIND,
};

fn body_id() -> BodyId {
    conduit_body::Body::born(
        SourceDocumentId::from("source/browser-mask-test"),
        CheckedFormId::from("checked/browser-mask-test"),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap()
    .body_id
}

fn presentation() -> Presentation {
    Presentation::new_with_interactions(
        7,
        PresentationBasis {
            body_id: Some(body_id()),
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from("source/application")),
            checked_form_id: Some(CheckedFormId::from("checked/application")),
            expanded_form_id: Some(ExpandedFormId::from("expanded/application")),
            plan_id: Some(PlanId::from("plan/application")),
            active_play_id: None,
            sign_ids: vec![SignId::from("sign/presentation")],
        },
        vec![PresentationSubject {
            identity: "body/browser-mask-test".into(),
            role: PresentationRole::Body,
            label: "Test Body".into(),
            accessibility_name: "Test Body".into(),
        }],
        vec![],
        vec![],
        vec![PresentationText {
            subject: "body/browser-mask-test".into(),
            text: "One canonical journey".into(),
        }],
        vec![PresentationAction {
            identity: "body.inspect".into(),
            intent: "conduit.intent/inspect@1".into(),
            target: "body/browser-mask-test".into(),
            label: "Inspect".into(),
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        vec![PresentationInput {
            identity: "input/inspect".into(),
            target: "body/browser-mask-test".into(),
            value_kind: UTF8_TEXT_VALUE_KIND.into(),
            maximum_bytes: 32,
            allow_empty: true,
            label: "Inspect".into(),
            accessibility_name: "Inspect".into(),
            submit_action: "body.inspect".into(),
        }],
        vec![],
    )
    .unwrap()
}

fn interaction(effect: &BrowserMaskEffect) -> BrowserMaskInteraction {
    BrowserMaskInteraction {
        show_id: effect.show_id.clone(),
        manifestation_id: effect.manifestation_id.clone(),
        presentation_id: effect.presentation_id.clone(),
        presentation_revision: effect.presentation_revision,
        input_id: "input/inspect".into(),
        action_id: "body.inspect".into(),
        target: "body/browser-mask-test".into(),
        value_kind: UTF8_TEXT_VALUE_KIND.into(),
        value: vec![],
        sequence: 1,
    }
}

fn acknowledgement(effect: &BrowserMaskEffect) -> BrowserMaskAcknowledgement {
    BrowserMaskAcknowledgement {
        show_id: effect.show_id.clone(),
        manifestation_id: effect.manifestation_id.clone(),
        mask_plan_id: effect.mask_plan_id.clone(),
        active_play_id: effect.mask_play.active_play_id.clone(),
        placement_id: effect.placement_id.clone(),
        presentation_id: effect.presentation_id.clone(),
        presentation_revision: effect.presentation_revision,
    }
}

fn body_plan_basis() -> (BodyId, conduit_body::Wake, conduit_body::BodyPlan) {
    let planned = plan::planned_mask(
        HostId::from("host/browser"),
        BootId::from("boot/browser"),
        plan::MASK_SOURCE,
        "browser-graphical",
    )
    .unwrap();
    let resident = conduit_body::ResidentForm::new(
        planned.mask.form_identity.source_document_id.clone(),
        planned.mask.form_identity.checked_form_id.clone(),
    );
    let born = conduit_body::Body::born(
        resident.source_document_id.clone(),
        resident.checked_form_id.clone(),
        1,
        SignId::from("sign/body-born-plan-basis"),
    )
    .unwrap();
    let body = born.body_id.clone();
    let (_, wake) = born.wake(1, SignId::from("sign/wake")).unwrap();
    let body_plan = conduit_body::BodyPlan::seal(
        &wake,
        vec![conduit_body::BodyFormPlan {
            form: resident,
            plan: planned.plan,
        }],
    )
    .unwrap();
    (body, wake, body_plan)
}

#[test]
fn show_becomes_available_only_after_exact_browser_acknowledgement() {
    let (body, wake, body_plan) = body_plan_basis();
    let (mut runtime, effect) = BrowserMaskRuntime::prepare(
        body,
        HostId::from("host/browser"),
        BootId::from("boot/browser"),
        presentation(),
        wake,
        body_plan,
    )
    .unwrap();
    assert_eq!(
        runtime.show.show.lifecycle,
        ManifestationLifecycle::Prepared
    );
    assert_ne!(
        runtime.presentation.basis.plan_id.as_ref(),
        Some(&runtime.planned.plan.plan_id)
    );
    let mut stale = acknowledgement(&effect);
    stale.presentation_revision += 1;
    assert!(runtime.acknowledge(&stale).is_err());
    assert_eq!(
        runtime.show.show.lifecycle,
        ManifestationLifecycle::Prepared
    );
    runtime.acknowledge(&acknowledgement(&effect)).unwrap();
    assert_eq!(
        runtime.show.show.lifecycle,
        ManifestationLifecycle::Available
    );
    assert_eq!(
        runtime.wardrobe_action.resulting_wardrobe.worn,
        vec![runtime.planned.mask.form_identity.clone()]
    );
    let mut stale_interaction = interaction(&effect);
    stale_interaction.presentation_revision += 1;
    assert!(runtime.interact(&stale_interaction).is_err());
    let receipt = runtime.interact(&interaction(&effect)).unwrap();
    assert_eq!(receipt.semantic_action.identity, "body.inspect");
    assert_eq!(
        receipt.correlation.interaction.manifestation_id,
        effect.manifestation_id
    );
    let observation = runtime.observation();
    assert_eq!(
        observation
            .interaction
            .as_ref()
            .unwrap()
            .correlation
            .interaction
            .identity,
        receipt.correlation.interaction.identity
    );
    assert_eq!(observation.execution.fore.len(), 3);
    let kinds = observation
        .execution
        .remote_signs
        .iter()
        .map(|sign| sign.kind.as_str())
        .collect::<Vec<_>>();
    for required in [
        "RemoteInputAdmitted",
        "RemoteInputClosed",
        "RemoteValueOffered",
        "RemoteValueAccepted",
        "RemoteValueDelivered",
        "RemoteOutputClosed",
    ] {
        assert!(kinds.contains(&required), "missing {required}: {kinds:?}");
    }

    let initial = runtime.observation();
    let (mut replacement, replacement_effect) = runtime
        .replacement(initial.wardrobe_action.body_id.clone())
        .unwrap();
    replacement
        .acknowledge(&acknowledgement(&replacement_effect))
        .unwrap();
    let journey = replacement.actualize_journey(&initial).unwrap();
    assert_eq!(journey.len(), 10);
    assert_eq!(
        journey
            .iter()
            .map(|outcome| outcome.action_id)
            .collect::<Vec<_>>(),
        conduit_presentation::MASK_JOURNEY_ACTIONS.map(conduit_presentation::MaskJourneyAction::id)
    );
    let initial_plan = &journey[0].plan_id;
    assert_eq!(&journey[2].plan_id, initial_plan);
    assert_eq!(journey[2].show_id, journey[0].show_id);
    for unavailable in &journey[3..=5] {
        assert_eq!(&unavailable.plan_id, initial_plan);
        assert!(unavailable.show_id.is_none());
    }
    assert_ne!(journey[6].plan_id, *initial_plan);
    assert_eq!(journey[7].plan_id, journey[6].plan_id);
    assert!(journey[7].show_id.is_some());
    assert!(journey[9].show_id.is_some());
}
