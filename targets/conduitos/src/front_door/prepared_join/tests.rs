use super::*;
use conduit_body::{Body, BodyBiographyEvidence, BodyMembership, PreparedBirthMediaBinding};

fn media(host: &str) -> PreparedBirthMedia {
    let body = Body::born(
        "source/retained".into(),
        "checked/retained".into(),
        41,
        "sign/born".into(),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence = BodyBiographyEvidence::born(body, membership, "Retained Cedar".into()).unwrap();
    PreparedBirthMedia::from_evidence(
        &evidence,
        PreparedBirthMediaBinding {
            host_entry: host.into(),
            first_host_entry: "first".into(),
            construction_id: "construction/exact".into(),
            configuration_id: "configuration/exact".into(),
            profile_id: "profile/native".into(),
            build_id: "build/native".into(),
            target: "target/native".into(),
        },
    )
    .unwrap()
}
fn door() -> FrontDoor {
    FrontDoor::new(
        "host/native".into(),
        "boot/fresh".into(),
        OfferGeneration(7),
        "profile/native",
        "build/native",
        "image/native",
        "source/entrance".into(),
        "checked/entrance".into(),
        1,
        true,
    )
}
#[test]
fn retained_later_host_is_a_candidate_without_invitation_membership_or_birth() {
    let media = media("later");
    let expected = media.clone();
    let mut door = door();
    door.await_prepared_join(media).unwrap();
    assert!(door.prepared_join_pending() && door.joining_pending());
    assert!(!door.lifecycle_authority_admitted);
    assert!(!door.creche_open());
    assert_eq!(door.prepared_join.as_ref(), Some(&expected));
    let face = door.face().unwrap();
    face.validate().unwrap();
    assert!(face.basis.body_id.is_none());
    assert!(
        face.basis.wake_id.is_none()
            && face.basis.plan_id.is_none()
            && face.basis.active_play_id.is_none()
    );
    assert!(face.actions.is_empty());
    assert!(
        face.subjects
            .iter()
            .any(|subject| subject.name == "Retained Cedar")
    );
    for name in [
        "invitation-available",
        "membership-admitted",
        "owner-receipt-verified",
    ] {
        assert!(
            face.properties.iter().any(|property| property.name == name
                && property.value == FacePropertyValue::Flag(false))
        );
    }
    assert!(
        face.properties
            .iter()
            .any(|property| property.name == "host-entry"
                && property.value == FacePropertyValue::Text("later".into()))
    );
    assert_eq!(
        door.open_creche("unused".into(), None),
        Err(Error::ActionUnavailable)
    );
    assert_eq!(
        door.await_prepared_join(expected.clone()),
        Err(Error::ActionUnavailable)
    );
    assert_eq!(door.prepared_join.as_ref(), Some(&expected));
}
#[test]
fn first_host_or_wrong_image_cannot_enter_retained_join_wait() {
    let mut door = door();
    assert_eq!(
        door.await_prepared_join(media("first")),
        Err(Error::ActionUnavailable)
    );
    assert!(!door.prepared_join_pending());
    door.profile_id = "different".into();
    assert_eq!(
        door.await_prepared_join(media("later")),
        Err(Error::ActionUnavailable)
    );
    assert!(!door.prepared_join_pending());
    assert!(door.lifecycle_authority_admitted);
}
