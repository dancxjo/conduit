#![cfg(feature = "plot-catalog")]
mod common;
use common::{available_mask_show, checked_renderer_plot, face, host, plan_for, WAYLAND_RESOURCE};
use conduit_face::*;

fn basis() -> (Face, MaskShow) {
    let plot = checked_renderer_plot();
    let plan = plan_for(
        &plot,
        host(
            "host",
            "boot",
            "renderer",
            "renderer@1",
            "artifact@1",
            "face/base/wayland-surface@1",
            WAYLAND_RESOURCE,
        ),
    );
    let base = face(&plot, &plan);
    let face = Face::new_with_semantics(
        base.revision,
        base.basis,
        base.subjects,
        base.relationships,
        base.properties,
        base.text,
        vec![FaceAction {
            identity: "send".into(),
            intent: "message/send@1".into(),
            target: "patchbay/plot".into(),
            name: "Send".into(),
            arguments: vec![
                FaceActionArgument::text("value".into(), "Message".into(), 1, 16).unwrap(),
            ],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        }],
        base.disclosures,
    )
    .unwrap();
    let show = available_mask_show(&face);
    (face, show)
}
fn arguments() -> Vec<FaceInteractionArgument> {
    vec![FaceInteractionArgument {
        name: "value".into(),
        value_kind: UTF8_TEXT_VALUE_KIND.into(),
        value: "α🦀".as_bytes().to_vec(),
    }]
}
fn interaction(face: &Face, show: &MaskShow) -> Result<FaceInteraction, FaceInteractionRefusal> {
    FaceInteraction::new(face, show, "send", "patchbay/plot", arguments(), 7)
}
#[test]
fn valid_public_boundaries_retain_exact_interaction_identity_and_encoding() {
    let (face, show) = basis();
    show.validate(&face).unwrap();
    let value = interaction(&face, &show).unwrap();
    let decoded = FaceInteraction::decode(&value.encode()).unwrap();
    assert_eq!(value, decoded);
    decoded.validate_against(&face, &show).unwrap();
    assert_eq!(value, interaction(&face, &show).unwrap());
    let json: FaceInteraction =
        serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(json.encode(), value.encode());
}
#[test]
fn public_entry_points_reject_mutated_face_content_and_identity_before_show() {
    let (face, show) = basis();
    let accepted = interaction(&face, &show).unwrap();
    for mutation in 0..3 {
        let mut invalid = face.clone();
        match mutation {
            0 => invalid.text[0].text.push('x'),
            1 => invalid.actions[0].arguments[0].contract.maximum_bytes += 1,
            _ => invalid.identity = serde_json::from_str("\"forged\"").unwrap(),
        }
        assert!(show.validate(&invalid).is_err());
        assert_eq!(
            interaction(&invalid, &show),
            Err(FaceInteractionRefusal::InvalidFace)
        );
        assert_eq!(
            accepted.validate_against(&invalid, &show),
            Err(if mutation == 2 {
                FaceInteractionRefusal::StaleFace
            } else {
                FaceInteractionRefusal::InvalidFace
            })
        );
    }
    let mut invalid = face;
    invalid.text[0].text.push('x');
    let mut wrong = show;
    wrong.planned_mask.plan.plan_id = "wrong".into();
    assert_eq!(
        interaction(&invalid, &wrong),
        Err(FaceInteractionRefusal::InvalidFace)
    );
}
#[test]
fn submitted_plan_show_and_lifecycle_cannot_bypass_public_validation() {
    let (face, show) = basis();
    for mutation in 0..4 {
        let mut wrong = show.clone();
        match mutation {
            0 => wrong.planned_mask.plan.plan_id = "wrong-plan".into(),
            1 => {
                wrong.planned_mask.plan.fragments[0].placements[0].artifact_id =
                    "substituted".into()
            }
            2 => wrong.show_id = serde_json::from_str("\"wrong-show\"").unwrap(),
            _ => wrong.show.boot_id = "replaced-boot".into(),
        }
        assert!(wrong.validate(&face).is_err());
        assert_eq!(
            interaction(&face, &wrong),
            Err(FaceInteractionRefusal::StaleShow)
        );
    }
    let failed = show
        .fail(ManifestationFailure::DeliveryFailed, "sign/failed".into())
        .unwrap();
    failed.validate(&face).unwrap();
    assert_eq!(
        interaction(&face, &failed),
        Err(FaceInteractionRefusal::FailedShow)
    );
    let closed = show
        .transition(ManifestationLifecycle::Closed, "sign/closed".into())
        .unwrap();
    closed.validate(&face).unwrap();
    assert_eq!(
        interaction(&face, &closed),
        Err(FaceInteractionRefusal::StaleShow)
    );
}
#[test]
fn exact_target_argument_contract_and_interaction_identity_remain_mandatory() {
    let (face, show) = basis();
    let value = interaction(&face, &show).unwrap();
    assert_eq!(
        FaceInteraction::new(&face, &show, "send", "other", arguments(), 7),
        Err(FaceInteractionRefusal::WrongTarget)
    );
    assert_eq!(
        FaceInteraction::new(&face, &show, "send", "patchbay/plot", vec![], 7),
        Err(FaceInteractionRefusal::MissingArgument)
    );
    let mut wrong_kind = arguments();
    wrong_kind[0].value_kind = "value/bool".into();
    assert_eq!(
        FaceInteraction::new(&face, &show, "send", "patchbay/plot", wrong_kind, 7),
        Err(FaceInteractionRefusal::WrongValueKind)
    );
    let mut oversized = arguments();
    oversized[0].value = vec![b'x'; 17];
    assert_eq!(
        FaceInteraction::new(&face, &show, "send", "patchbay/plot", oversized, 7),
        Err(FaceInteractionRefusal::OversizeValue)
    );
    let mut forged = value.clone();
    forged.arguments[0].value = b"different".to_vec();
    assert_eq!(
        forged.validate_against(&face, &show),
        Err(FaceInteractionRefusal::MalformedEncoding)
    );
    let fresh_face = Face::new_with_semantics(
        face.revision + 1,
        face.basis.clone(),
        face.subjects.clone(),
        face.relationships.clone(),
        face.properties.clone(),
        face.text.clone(),
        face.actions.clone(),
        face.disclosures.clone(),
    )
    .unwrap();
    assert_eq!(
        interaction(&fresh_face, &show),
        Err(FaceInteractionRefusal::StaleShow)
    );
    assert_eq!(
        value.validate_against(&fresh_face, &show),
        Err(FaceInteractionRefusal::StaleFace)
    );
    let mut stale = value.clone();
    stale.face_revision += 1;
    assert_eq!(
        stale.validate_against(&face, &show),
        Err(FaceInteractionRefusal::StaleFace)
    );
    let mut stale = value;
    stale.show_id = "other".into();
    assert_eq!(
        stale.validate_against(&face, &show),
        Err(FaceInteractionRefusal::StaleShow)
    );
}
