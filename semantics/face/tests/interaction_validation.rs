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

fn constructed(face: Face) -> Result<CheckedFace, FaceError> {
    CheckedFace::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
}

#[test]
fn constructor_proof_matches_public_validation_for_exact_and_adversarial_inputs() {
    let (face, show) = basis();
    let checked = constructed(face.clone()).unwrap();
    assert_eq!(checked.as_face(), &face);
    assert_eq!(
        serde_json::to_vec(checked.as_face()).unwrap(),
        serde_json::to_vec(&face).unwrap()
    );
    let valid = interaction(&face, &show).unwrap();
    let roundtrip = FaceInteraction::decode(&valid.encode()).unwrap();
    assert_eq!(checked.validate_interaction(&roundtrip, &show), Ok(()));
    for mutation in 0..10 {
        let mut input = valid.clone();
        match mutation {
            0 => input.face_revision += 1,
            1 => input.show_id = "stale".into(),
            2 => input.action_id = "unknown".into(),
            3 => input.target = "wrong".into(),
            4 => input.arguments.clear(),
            5 => input.arguments[0].value_kind = "value/bool".into(),
            6 => input.arguments[0].value = vec![b'x'; 17],
            7 => input.arguments.push(input.arguments[0].clone()),
            8 => input.arguments[0].value = b"changed".to_vec(),
            _ => input.identity = serde_json::from_str("\"forged\"").unwrap(),
        }
        let expected = input.validate_against(&face, &show);
        assert!(expected.is_err());
        assert_eq!(checked.validate_interaction(&input, &show), expected);
    }
    for mutation in 0..5 {
        let mut wrong = show.clone();
        match mutation {
            0 => wrong.planned_mask.plan.plan_id = "wrong".into(),
            1 => wrong.planned_mask.plan.fragments[0].placements[0].artifact_id = "wrong".into(),
            2 => wrong.show.boot_id = "wrong".into(),
            3 => {
                wrong = show
                    .fail(ManifestationFailure::DeliveryFailed, "sign/failed".into())
                    .unwrap()
            }
            _ => {
                wrong = show
                    .transition(ManifestationLifecycle::Closed, "sign/closed".into())
                    .unwrap()
            }
        }
        let expected = valid.validate_against(&face, &wrong);
        assert!(expected.is_err());
        assert_eq!(checked.validate_interaction(&valid, &wrong), expected);
    }
    let mut unwrapped = checked.into_face();
    unwrapped.text[0].text.push('x');
    assert_eq!(
        valid.validate_against(&unwrapped, &show),
        Err(FaceInteractionRefusal::InvalidFace)
    );
}

#[test]
fn checked_constructor_retains_content_refusals() {
    let (mut face, _) = basis();
    face.subjects.push(face.subjects[0].clone());
    assert_eq!(constructed(face).unwrap_err(), FaceError::DuplicateSubject);
    let (mut face, _) = basis();
    face.actions[0].arguments[0].contract.maximum_bytes = u32::MAX;
    assert_eq!(
        constructed(face).unwrap_err(),
        FaceError::InvalidInputContract
    );
}

#[test]
fn borrowed_validation_retains_conflicting_refusal_precedence() {
    let (face, show) = basis();
    let accepted = interaction(&face, &show).unwrap();
    let checked = constructed(face.clone()).unwrap();
    let check = |value: &FaceInteraction, expected| {
        assert_eq!(value.validate_against(&face, &show), Err(expected));
        assert_eq!(checked.validate_interaction(value, &show), Err(expected));
    };
    let mut value = accepted.clone();
    value.identity = serde_json::from_str("\"forged\"").unwrap();
    value.target = "wrong".into();
    value.arguments.clear();
    check(&value, FaceInteractionRefusal::WrongTarget);
    value = accepted.clone();
    value.arguments[0].value_kind = "wrong".into();
    value.arguments[0].value.resize(17, b'x');
    value.arguments.push(value.arguments[0].clone());
    check(&value, FaceInteractionRefusal::DuplicateArgument);
    value.arguments.pop();
    check(&value, FaceInteractionRefusal::WrongValueKind);
    value.arguments[0].value_kind = UTF8_TEXT_VALUE_KIND.into();
    value.identity = serde_json::from_str("\"forged\"").unwrap();
    check(&value, FaceInteractionRefusal::OversizeValue);
    value = accepted;
    for identity in [
        "".to_string(),
        "a".repeat(63),
        "a".repeat(65),
        "a".repeat(8193),
    ] {
        value.identity = serde_json::from_value(serde_json::Value::String(identity)).unwrap();
        check(&value, FaceInteractionRefusal::MalformedEncoding);
    }
}

#[test]
fn constructor_and_borrowed_validation_share_exact_wire_size_boundary() {
    let (base, _) = basis();
    let mut actions = base.actions.clone();
    actions[0].arguments = ["first", "second"]
        .into_iter()
        .map(|name| FaceActionArgument::text(name.into(), name.into(), 1, 4096).unwrap())
        .collect();
    let face = Face::new_with_semantics(
        base.revision,
        base.basis,
        base.subjects,
        base.relationships,
        base.properties,
        base.text,
        actions,
        base.disclosures,
    )
    .unwrap();
    let show = available_mask_show(&face);
    let checked = constructed(face.clone()).unwrap();
    let mut args: Vec<_> = ["first", "second"]
        .into_iter()
        .map(|name| FaceInteractionArgument {
            name: name.into(),
            value_kind: UTF8_TEXT_VALUE_KIND.into(),
            value: vec![b'x'],
        })
        .collect();
    let small =
        FaceInteraction::new(&face, &show, "send", "patchbay/plot", args.clone(), 7).unwrap();
    let remaining = MAX_FACE_INTERACTION_BYTES - small.encode().len();
    args[0].value.resize(4096, b'x');
    args[1].value.resize(1 + remaining - 4095, b'x');
    let exact =
        FaceInteraction::new(&face, &show, "send", "patchbay/plot", args.clone(), 7).unwrap();
    assert_eq!(exact.encode().len(), MAX_FACE_INTERACTION_BYTES);
    assert_eq!(exact.validate_against(&face, &show), Ok(()));
    assert_eq!(checked.validate_interaction(&exact, &show), Ok(()));
    let mut forged = exact.clone();
    forged.identity = serde_json::from_value(serde_json::Value::String("a".repeat(8193))).unwrap();
    // Size validation uses a derived 64-byte identity, not the submitted ID length.
    assert_eq!(
        forged.validate_against(&face, &show),
        Err(FaceInteractionRefusal::MalformedEncoding)
    );
    forged.arguments[1].value.push(b'x');
    assert_eq!(
        forged.validate_against(&face, &show),
        Err(FaceInteractionRefusal::OversizeValue)
    );
    assert_eq!(
        checked.validate_interaction(&forged, &show),
        Err(FaceInteractionRefusal::OversizeValue)
    );
    args[1].value.push(b'x');
    assert_eq!(
        FaceInteraction::new(&face, &show, "send", "patchbay/plot", args, 7),
        Err(FaceInteractionRefusal::OversizeValue)
    );
}

#[test]
fn borrowed_show_basis_keeps_partial_provenance_and_public_refusal_order() {
    let (original, _) = basis();
    for variant in 0..4 {
        let mut provenance = original.basis.clone();
        if variant != 0 {
            provenance.plan_id = None;
            provenance.active_play_id = None;
        }
        if variant == 1 || variant == 3 {
            provenance.expanded_plot_id = None;
        }
        if variant == 2 || variant == 3 {
            provenance.source_document_id = None;
            provenance.checked_plot_id = None;
        }
        let face = original.clone().with_basis(provenance).unwrap();
        let show = available_mask_show(&face);
        assert_eq!(show.face_plot.is_some(), variant == 0);
        show.validate(&face).unwrap();
        interaction(&face, &show).unwrap();
        let mut wrong = show.clone();
        wrong.face_plot = if wrong.face_plot.is_some() {
            None
        } else {
            Some(wrong.mask_plot.clone())
        };
        assert_eq!(wrong.validate(&face), Err(MaskShowError::StalePlan));
        let mut wrong = show.clone();
        wrong.face_plan_id = if wrong.face_plan_id.is_some() {
            None
        } else {
            Some("unexpected-plan".into())
        };
        assert_eq!(wrong.validate(&face), Err(MaskShowError::StalePlan));
        let mut wrong = show;
        wrong.show_id = serde_json::from_str("\"show/not-canonical\"").unwrap();
        assert_eq!(wrong.validate(&face), Err(MaskShowError::StaleShowIdentity));
        wrong.planned_mask.plan.plan_id = "wrong-plan".into();
        assert_eq!(
            wrong.validate(&face),
            Err(MaskShowError::InvalidManifestation(
                ManifestationError::InvalidPlan
            ))
        );
    }
}
