use super::*;
use conduit_core::{CheckedFormId, ExpandedFormId, PlanId, SourceDocumentId};
use conduit_presentation::{
    PresentationBasis, PresentationRole, PresentationSubject, PresentationText,
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
    Presentation::new(
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
    )
    .unwrap()
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

#[test]
fn show_becomes_available_only_after_exact_browser_acknowledgement() {
    let (mut runtime, effect) = BrowserMaskRuntime::prepare(
        body_id(),
        HostId::from("host/browser"),
        BootId::from("boot/browser"),
        presentation(),
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
    let observation = runtime.observation();
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
}
