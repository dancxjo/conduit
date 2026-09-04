use alloc::vec;
use conduit_body::Body;
use conduit_core::{
    ActivePlayId, BaseImplementationId, CheckedFormId, ExpandedFormId, PlanId, SignId,
    SourceDocumentId,
};

use super::*;
use crate::{
    PresentationBasis, PresentationProperty, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationText,
};

fn exact_presentation() -> Presentation {
    let source_document_id = SourceDocumentId::from("source/interface-parity");
    let checked_form_id = CheckedFormId::from("checked/interface-parity");
    let body = Body::born(
        source_document_id.clone(),
        checked_form_id.clone(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let (body, wake) = body.wake(1, SignId::from("sign/woke")).unwrap();
    Presentation::new(
        7,
        PresentationBasis {
            body_id: Some(body.body_id),
            wake_id: Some(wake.wake_id),
            source_document_id: Some(source_document_id),
            checked_form_id: Some(checked_form_id),
            expanded_form_id: Some(ExpandedFormId::from("expanded/interface-parity")),
            plan_id: Some(PlanId::from("plan/interface-parity")),
            active_play_id: Some(ActivePlayId::from("play/interface-parity")),
            sign_ids: vec![SignId::from("sign/playing"), SignId::from("sign/presented")],
        },
        vec![
            PresentationSubject {
                identity: "subject/body".into(),
                role: PresentationRole::Document,
                label: "Body view".into(),
                accessibility_name: "Exact Body and Wake view".into(),
            },
            PresentationSubject {
                identity: "subject/play".into(),
                role: PresentationRole::Play,
                label: "Active Play".into(),
                accessibility_name: "Active Play status".into(),
            },
        ],
        vec![PresentationRelationship {
            source: "subject/body".into(),
            target: "subject/play".into(),
            kind: PresentationRelationshipKind::Contains,
        }],
        vec![PresentationProperty {
            subject: "subject/play".into(),
            name: "line-base".into(),
            value: PresentationPropertyValue::BaseImplementationId(BaseImplementationId::from(
                "conduit.base/usb-cdc-acm@1",
            )),
        }],
        vec![PresentationText {
            subject: "subject/play".into(),
            text: "line one\nline two".into(),
        }],
    )
    .unwrap()
}

#[test]
fn linear_projection_preserves_exact_basis_and_all_nonspatial_content() {
    let presentation = exact_presentation();
    let linear = render_linear_presentation(&presentation).unwrap();
    let output = linear.lines.join("\n");

    assert_eq!(linear.presentation_id, presentation.identity);
    assert_eq!(linear.revision, presentation.revision);
    for identity in [
        presentation.identity.as_str(),
        presentation.basis.body_id.as_ref().unwrap().as_str(),
        presentation.basis.wake_id.as_ref().unwrap().as_str(),
        presentation
            .basis
            .source_document_id
            .as_ref()
            .unwrap()
            .as_str(),
        presentation
            .basis
            .checked_form_id
            .as_ref()
            .unwrap()
            .as_str(),
        presentation
            .basis
            .expanded_form_id
            .as_ref()
            .unwrap()
            .as_str(),
        presentation.basis.plan_id.as_ref().unwrap().as_str(),
        presentation.basis.active_play_id.as_ref().unwrap().as_str(),
        presentation.basis.sign_ids[0].as_str(),
        presentation.basis.sign_ids[1].as_str(),
    ] {
        assert!(output.contains(identity));
    }
    assert!(output.contains(
        "SUBJECT role=Play id=\"subject/play\" label=\"Active Play\" accessibility=\"Active Play status\""
    ));
    assert!(output
        .contains("RELATIONSHIP kind=Contains source=\"subject/body\" target=\"subject/play\""));
    assert!(output
        .contains("PROPERTY subject=\"subject/play\" name=\"line-base\" value=base=conduit.base/usb-cdc-acm@1"));
    assert!(output.contains("TEXT subject=\"subject/play\" value=\"line one\\nline two\""));
    assert!(linear.lines.len() <= MAX_LINEAR_PRESENTATION_LINES);
    assert!(linear.encoded_bytes <= MAX_LINEAR_PRESENTATION_BYTES);
    assert_eq!(
        linear.encoded_bytes,
        linear.lines.iter().map(|line| line.len() + 1).sum()
    );
}

#[test]
fn invalid_content_identity_fails_closed_before_rendering() {
    let mut presentation = exact_presentation();
    presentation.revision += 1;
    assert_eq!(
        render_linear_presentation(&presentation),
        Err(LinearPresentationError::InvalidPresentation(
            PresentationError::InvalidIdentity
        ))
    );
}

#[test]
fn rendering_is_deterministic_and_contains_no_renderer_local_input() {
    let presentation = exact_presentation();
    let first = render_linear_presentation(&presentation).unwrap();
    let renderer_local_state = ("wayland-window-9", 640_u16, 480_u16);
    assert_eq!(
        first,
        render_linear_presentation(&presentation).unwrap(),
        "renderer-local state {renderer_local_state:?} has no projection input"
    );
}
