use super::*;
use conduit_body::{
    BodyBiographyEvidence, BodyMembership, BodyWorkset, PreparedBirthMediaBinding, ResidentPlot,
};
fn journey() -> ProductJourney {
    ProductJourney::new(
        "host/current".into(),
        "boot/fresh".into(),
        OfferGeneration(7),
    )
    .unwrap()
}
fn media(host_entry: &str, workset: BodyWorkset, sequence: u64) -> PreparedBirthMedia {
    let body = Body::born_with_plots(workset, sequence, "sign/prepared-birth".into()).unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence = BodyBiographyEvidence::born(body, membership, "Fir Compass".into()).unwrap();
    PreparedBirthMedia::from_evidence(
        &evidence,
        PreparedBirthMediaBinding {
            host_entry: host_entry.into(),
            first_host_entry: "first".into(),
            construction_id: "construction/exact".into(),
            configuration_id: "configuration/exact".into(),
            profile_id: "profile/exact".into(),
            build_id: "build/exact".into(),
            target: "target/exact".into(),
        },
    )
    .unwrap()
}
fn installed() -> BodyWorkset {
    BodyWorkset::one(native_workset::resident(NativePlot::KeyboardCanvas).unwrap()).unwrap()
}
#[test]
fn actual_import_retains_birth_and_admits_only_current_host_boot() {
    let media = media("first", installed(), 41);
    let mut journey = journey();
    journey
        .import_prepared_birth(&media, "target/exact", "profile/exact", "build/exact")
        .unwrap();
    let biography = journey.biography().unwrap();
    biography.validate().unwrap();
    assert_eq!(biography.body_id, media.evidence().body_id);
    assert_eq!(biography.friendly_name, media.evidence().friendly_name);
    assert_eq!(biography.body, media.evidence().body);
    assert_eq!(biography.records[0], media.evidence().records[0]);
    assert_eq!(biography.body.sign_ids, media.evidence().body.sign_ids);
    assert_eq!(biography.membership.parts.len(), 1);
    let actual = biography.membership.parts[0].current.as_ref().unwrap();
    assert_eq!(actual.host_id.as_str(), "host/current");
    assert_eq!(actual.boot_id.as_str(), "boot/fresh");
    assert_eq!(actual.offer_generation, OfferGeneration(7));
    assert!(
        biography
            .records
            .iter()
            .skip(1)
            .all(|record| record.sequence > 41)
    );
    assert!(biography.wakes.is_empty());
    assert_eq!(journey.status(), JourneyStatus::BornLulled);
    assert!(journey.current_plan().is_none() && journey.current_play().is_none());
    let before = journey.projection();
    assert_eq!(
        journey.import_prepared_birth(&media, "target/exact", "profile/exact", "build/exact"),
        Err(JourneyError::AlreadyBorn)
    );
    assert_eq!(journey.projection(), before);
}
#[test]
fn wrong_artifact_binding_later_host_and_unknown_workset_are_atomic_refusals() {
    let valid = media("first", installed(), 41);
    for (target, profile, build) in [
        ("wrong", "profile/exact", "build/exact"),
        ("target/exact", "wrong", "build/exact"),
        ("target/exact", "profile/exact", "wrong"),
    ] {
        let mut journey = journey();
        let before = journey.projection();
        assert_eq!(
            journey.import_prepared_birth(&valid, target, profile, build),
            Err(JourneyError::WrongTarget)
        );
        assert_eq!(journey.projection(), before);
    }
    let unknown = BodyWorkset::one(ResidentPlot::new(
        "source/unsupported".into(),
        "checked/unsupported".into(),
    ))
    .unwrap();
    for (candidate, expected) in [
        (
            media("later", installed(), 41),
            JourneyError::AdmissionUnsupported,
        ),
        (media("first", unknown, 41), JourneyError::WrongTarget),
    ] {
        let mut journey = journey();
        let before = journey.projection();
        assert_eq!(
            journey.import_prepared_birth(
                &candidate,
                "target/exact",
                "profile/exact",
                "build/exact"
            ),
            Err(expected)
        );
        assert_eq!(journey.projection(), before);
        assert!(journey.body().is_none());
    }
}
#[test]
fn membership_sequence_overflow_and_nonworld_import_leave_state_unchanged() {
    let overflow = media("first", installed(), u64::MAX);
    let mut journey = journey();
    let before = journey.projection();
    assert_eq!(
        journey.import_prepared_birth(&overflow, "target/exact", "profile/exact", "build/exact"),
        Err(JourneyError::Biography(
            conduit_body::BodyBiographyError::InvalidSequence
        ))
    );
    assert_eq!(journey.projection(), before);
    journey.open_plot().unwrap();
    let before = journey.projection();
    let valid = media("first", installed(), 41);
    assert_eq!(
        journey.import_prepared_birth(&valid, "target/exact", "profile/exact", "build/exact"),
        Err(JourneyError::InvalidTransition)
    );
    assert_eq!(journey.projection(), before);
}
