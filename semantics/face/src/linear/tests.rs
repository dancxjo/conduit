use alloc::vec;
use conduit_body::Body;
use conduit_core::{
    ActivePlayId, BaseImplementationId, CheckedPlotId, ExpandedPlotId, PlanId, SignId,
    SourceDocumentId,
};

use super::*;
use crate::{
    FaceBasis, FaceProperty, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject,
    FaceText,
};

fn exact_face() -> Face {
    let source_document_id = SourceDocumentId::from("source/interface-parity");
    let checked_plot_id = CheckedPlotId::from("checked/interface-parity");
    let body = Body::born(
        source_document_id.clone(),
        checked_plot_id.clone(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let (body, wake) = body.wake(1, SignId::from("sign/woke")).unwrap();
    Face::new(
        7,
        FaceBasis {
            body_id: Some(body.body_id),
            wake_id: Some(wake.wake_id),
            source_document_id: Some(source_document_id),
            checked_plot_id: Some(checked_plot_id),
            expanded_plot_id: Some(ExpandedPlotId::from("expanded/interface-parity")),
            plan_id: Some(PlanId::from("plan/interface-parity")),
            active_play_id: Some(ActivePlayId::from("play/interface-parity")),
            sign_ids: vec![SignId::from("sign/playing"), SignId::from("sign/presented")],
        },
        vec![
            FaceSubject {
                identity: "subject/body".into(),
                role: FaceRole::Document,
                name: "Exact body and Wake view".into(),
            },
            FaceSubject {
                identity: "subject/play".into(),
                role: FaceRole::Play,
                name: "Active play status".into(),
            },
        ],
        vec![FaceRelationship {
            source: "subject/body".into(),
            target: "subject/play".into(),
            kind: FaceRelationshipKind::Contains,
        }],
        vec![FaceProperty {
            subject: "subject/play".into(),
            name: "line-base".into(),
            value: FacePropertyValue::BaseImplementationId(BaseImplementationId::from(
                "conduit.base/usb-cdc-acm@1",
            )),
        }],
        vec![FaceText {
            subject: "subject/play".into(),
            text: "line one\nline two".into(),
        }],
    )
    .unwrap()
}

#[test]
fn linear_projection_preserves_exact_basis_and_all_nonspatial_content() {
    let face = exact_face();
    let linear = render_linear_face(&face).unwrap();
    let output = linear.lines.join("\n");

    assert_eq!(linear.face_id, face.identity);
    assert_eq!(linear.revision, face.revision);
    for identity in [
        face.identity.as_str(),
        face.basis.body_id.as_ref().unwrap().as_str(),
        face.basis.wake_id.as_ref().unwrap().as_str(),
        face.basis.source_document_id.as_ref().unwrap().as_str(),
        face.basis.checked_plot_id.as_ref().unwrap().as_str(),
        face.basis.expanded_plot_id.as_ref().unwrap().as_str(),
        face.basis.plan_id.as_ref().unwrap().as_str(),
        face.basis.active_play_id.as_ref().unwrap().as_str(),
        face.basis.sign_ids[0].as_str(),
        face.basis.sign_ids[1].as_str(),
    ] {
        assert!(output.contains(identity));
    }
    assert!(output.contains("SUBJECT role=Play id=\"subject/play\" name=\"Active play status\""));
    assert!(output
        .contains("RELATIONSHIP kind=Contains source=\"subject/body\" target=\"subject/play\""));
    assert!(output
        .contains("PROPERTY subject=\"subject/play\" name=\"line-base\" value=base=conduit.base/usb-cdc-acm@1"));
    assert!(output.contains("TEXT subject=\"subject/play\" value=\"line one\\nline two\""));
    assert!(linear.lines.len() <= MAX_LINEAR_FACE_LINES);
    assert!(linear.encoded_bytes <= MAX_LINEAR_FACE_BYTES);
    assert_eq!(
        linear.encoded_bytes,
        linear
            .lines
            .iter()
            .map(|line| line.len() + 1)
            .sum::<usize>()
    );
}

#[test]
fn invalid_content_identity_fails_closed_before_rendering() {
    let mut face = exact_face();
    face.revision += 1;
    assert_eq!(
        render_linear_face(&face),
        Err(LinearFaceError::InvalidFace(FaceError::InvalidIdentity))
    );
}

#[test]
fn rendering_is_deterministic_and_contains_no_renderer_local_input() {
    let face = exact_face();
    let first = render_linear_face(&face).unwrap();
    let renderer_local_state = ("wayland-window-9", 640_u16, 480_u16);
    assert_eq!(
        first,
        render_linear_face(&face).unwrap(),
        "renderer-local state {renderer_local_state:?} has no projection input"
    );
}
