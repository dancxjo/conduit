use super::{common::available_mask_show, fixtures::*};
use conduit_face::*;

#[test]
fn exclusive_proposal_uses_exact_source_bytes_and_argument_not_duplicate_labels() {
    let face = specimen(Some(ChoiceMultiplicity::Exclusive), 2);
    let before = face.clone();
    let prepared = prepare_face_collection(&face, "list", 64).unwrap();
    let show = available_mask_show(&face);
    let input = prepared
        .interaction(&face, &show, "opaque/1", true, 17)
        .unwrap();
    assert_eq!(input.action_id, "source/replace");
    assert_eq!(input.target, "source");
    assert_eq!(input.arguments[0].name, "exact/source-argument");
    assert_eq!(input.arguments[0].value_kind, "value/text");
    assert_eq!(input.arguments[0].value, b"value/1");
    assert_eq!(input.sequence, 17);
    assert_eq!(input.show_id, show.show_id.as_str());
    input.validate_against(&face, &show).unwrap();
    assert_eq!(
        prepared.interaction(&face, &show, "opaque/1", false, 18),
        Err(FaceCollectionRefusal::Selection)
    );
    assert_eq!(
        prepared.interaction(&face, &show, "同じ · étiquette", true, 18),
        Err(FaceCollectionRefusal::UnknownItem)
    );
    assert_eq!(face, before);
}

#[test]
fn independent_replacements_are_explicit_and_informational_lists_cannot_select() {
    let face = specimen(Some(ChoiceMultiplicity::Independent), 2);
    let prepared = prepare_face_collection(&face, "list", 64).unwrap();
    let show = available_mask_show(&face);
    for choose in [false, true] {
        let input = prepared
            .interaction(&face, &show, "opaque/2", choose, 1)
            .unwrap();
        assert_eq!(input.target, "opaque/2");
        assert_eq!(input.action_id, "source/replace/0");
        assert_eq!(input.arguments[0].name, "exact/source-boolean");
        assert_eq!(input.arguments[0].value, [u8::from(choose)]);
    }
    let face = specimen(None, 1);
    let prepared = prepare_face_collection(&face, "list", 64).unwrap();
    let show = available_mask_show(&face);
    assert_eq!(
        prepared.interaction(&face, &show, "opaque/1", true, 1),
        Err(FaceCollectionRefusal::NoSelectionAction)
    );
}

#[test]
fn current_face_and_acknowledged_show_are_required_before_proposing() {
    let face = specimen(Some(ChoiceMultiplicity::Independent), 2);
    let prepared = prepare_face_collection(&face, "list", 64).unwrap();
    let show = available_mask_show(&face);
    let mut newer = face.clone();
    newer.revision += 1;
    let newer = rebuild(newer);
    assert_eq!(
        prepared.interaction(&newer, &show, "opaque/2", true, 1),
        Err(FaceCollectionRefusal::StaleFace)
    );
    let newer_show = available_mask_show(&newer);
    assert!(matches!(
        prepared.interaction(&face, &newer_show, "opaque/2", true, 1),
        Err(FaceCollectionRefusal::Interaction(_))
    ));
    let closed = show
        .transition(ManifestationLifecycle::Closed, "fixture/closed".into())
        .unwrap();
    assert!(matches!(
        prepared.interaction(&face, &closed, "opaque/2", true, 1),
        Err(FaceCollectionRefusal::Interaction(_))
    ));
    let mut forged = show.clone();
    forged.face_revision += 1;
    assert!(matches!(
        prepared.interaction(&face, &forged, "opaque/2", true, 1),
        Err(FaceCollectionRefusal::Interaction(_))
    ));
}

#[test]
fn unavailable_and_refused_actions_keep_readable_members_without_authority() {
    for availability in [
        FaceActionAvailability::Unavailable {
            reason_code: "fixture/disabled".into(),
            explanation: "Not available".into(),
        },
        FaceActionAvailability::Refused {
            reason_code: "fixture/refused".into(),
            explanation: "Refused".into(),
        },
    ] {
        let mut face = specimen(Some(ChoiceMultiplicity::Independent), 2);
        face.actions[0].availability = availability;
        let face = rebuild(face);
        let before = face.clone();
        let prepared = prepare_face_collection(&face, "list", 64).unwrap();
        assert_eq!(prepared.items().len(), 2);
        let show = available_mask_show(&face);
        assert!(matches!(
            prepared.interaction(&face, &show, "opaque/2", false, 1),
            Err(FaceCollectionRefusal::Interaction(
                FaceInteractionRefusal::UnavailableAction | FaceInteractionRefusal::RefusedAction
            ))
        ));
        assert_eq!(face, before);
    }
}
