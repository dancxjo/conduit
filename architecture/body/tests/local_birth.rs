use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyLifecycleSession,
    BodyLifecycleSessionError, BodyMembership, BodyState, BodyWorkset, MembershipProofId, PartId,
    ResidentPlot,
};
use conduit_core::{BootId, HostId, OfferGeneration, SignId};

fn birth(sequence: u64, workset: BodyWorkset) -> BodyBiographyEvidence {
    let body = Body::born_with_plots(workset, sequence, "operator/birth".into()).unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    BodyBiographyEvidence::born(body, membership, "One Body".into()).unwrap()
}

fn workset() -> BodyWorkset {
    BodyWorkset::one(ResidentPlot::new("source/one".into(), "checked/one".into())).unwrap()
}

fn observation(host: &str, boot: &str) -> AuthenticatedHostObservation {
    AuthenticatedHostObservation {
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(3),
        proof_id: MembershipProofId::bind("caller/verified-local-birth").unwrap(),
        sequence: 0,
    }
}

fn prepare(
    birth: &BodyBiographyEvidence,
    observation: AuthenticatedHostObservation,
) -> Result<BodyLifecycleSession, BodyLifecycleSessionError> {
    BodyLifecycleSession::prepare_first_host(
        birth,
        PartId::bind(&birth.body_id, observation.host_id.as_str(), 0).unwrap(),
        observation,
        "caller/admitted".into(),
        "caller/attached".into(),
    )
}

/// Independent reference to the two callers' previous explicit assembly.
fn prior_assembly(
    birth: &BodyBiographyEvidence,
    observed: AuthenticatedHostObservation,
) -> BodyBiographyEvidence {
    let mut membership = birth.membership.clone();
    let part = PartId::bind(&birth.body_id, observed.host_id.as_str(), 0).unwrap();
    let admitted = membership
        .admit(
            &birth.body_id,
            membership.revision,
            part.clone(),
            observed.proof_id.clone(),
            "caller/admitted".into(),
        )
        .unwrap();
    let attached = membership
        .observe_present(
            &birth.body_id,
            membership.revision,
            &part,
            observed,
            "caller/attached".into(),
        )
        .unwrap();
    let mut evidence = birth.clone();
    evidence
        .append_membership_events(
            membership,
            &[
                (admitted, birth.body.birth_sequence + 1),
                (attached, birth.body.birth_sequence + 2),
            ],
        )
        .unwrap();
    evidence
}

#[test]
fn both_host_profiles_preserve_the_existing_exact_birth_and_event_order() {
    let birth = birth(17, workset());
    for (host, boot) in [("linux/host", "linux/boot"), ("native/host", "native/boot")] {
        let observation = observation(host, boot);
        let expected = prior_assembly(&birth, observation.clone());
        let session = prepare(&birth, observation.clone()).unwrap();
        assert_eq!(session.evidence(), &expected);
        assert_eq!(session.evidence().body, birth.body);
        assert_eq!(session.evidence().records[0], birth.records[0]);
        assert_eq!(session.evidence().body.state, BodyState::Lulled);
        assert_eq!(session.evidence().membership.parts.len(), 1);
        assert_eq!(session.evidence().membership.events.len(), 2);
        assert_eq!(session.evidence().records.len(), 3);
        assert_eq!(session.evidence().wakes.len(), 0);
        assert!(session.realization().is_none());
        assert_eq!(session.pending_archives().len(), 0);
        assert_eq!(
            session.evidence().membership.parts[0].current.as_ref(),
            Some(&observation)
        );
    }
    assert_eq!(birth.membership.parts.len(), 0);
    assert_eq!(birth.records.len(), 1);
}

#[test]
fn decoded_prepared_birth_keeps_identity_name_and_workset_without_rebirth() {
    let original = birth(4, workset());
    let decoded = serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    let session = prepare(&decoded, observation("host", "boot")).unwrap();
    assert_eq!(session.evidence().body_id, original.body_id);
    assert_eq!(session.evidence().body, original.body);
    assert_eq!(session.evidence().friendly_name, original.friendly_name);
    assert_eq!(session.evidence().body.events.len(), 1);
}

#[test]
fn empty_workset_is_a_lulled_body_not_an_implicit_startup_plot() {
    let birth = birth(4, BodyWorkset::default());
    let session = prepare(&birth, observation("host", "boot")).unwrap();
    assert!(session.evidence().body.workset.is_empty());
    assert!(session.foreground().is_none());
    assert!(session.realization().is_none());
}

#[test]
fn first_host_preparation_cannot_be_used_as_return_or_additional_member_admission() {
    let birth = birth(4, workset());
    let session = prepare(&birth, observation("host", "boot")).unwrap();
    let before = session.evidence().clone();
    for observed in [
        observation("host", "boot"),
        observation("host", "replacement-boot"),
        observation("second-host", "second-boot"),
    ] {
        assert!(matches!(
            prepare(&before, observed),
            Err(BodyLifecycleSessionError::Biography(_))
        ));
        assert_eq!(session.evidence(), &before);
    }
}

#[test]
fn invalid_current_observation_refuses_without_changing_the_birth() {
    let birth = birth(4, workset());
    let before = birth.clone();
    let mut missing_proof = observation("host", "boot");
    missing_proof.proof_id = serde_json::from_str("\"\"").unwrap();
    for invalid in [
        observation("host", ""),
        observation("", "boot"),
        missing_proof,
    ] {
        assert!(BodyLifecycleSession::prepare_first_host(
            &birth,
            PartId::bind(&birth.body_id, "local-part", 0).unwrap(),
            invalid,
            "caller/admitted".into(),
            "caller/attached".into(),
        )
        .is_err());
    }
    assert_eq!(birth, before);
}

#[test]
fn malformed_birth_is_not_repaired_or_replaced_during_preparation() {
    let mut birth = birth(4, workset());
    birth.friendly_name.clear();
    let before = birth.clone();
    assert!(matches!(
        prepare(&birth, observation("host", "boot")),
        Err(BodyLifecycleSessionError::Biography(_))
    ));
    assert_eq!(birth, before);
}

#[test]
fn chronology_exhaustion_is_an_explicit_nonmutating_refusal() {
    for sequence in [u64::MAX - 1, u64::MAX] {
        let birth = birth(sequence, workset());
        let before = birth.clone();
        assert!(matches!(
            prepare(&birth, observation("host", "boot")),
            Err(BodyLifecycleSessionError::SequenceExhausted)
        ));
        assert_eq!(birth, before);
    }
    let birth = birth(u64::MAX - 2, workset());
    let session = prepare(&birth, observation("host", "boot")).unwrap();
    assert_eq!(session.evidence().records[2].sequence, u64::MAX);
}

#[test]
fn duplicate_signs_cannot_publish_a_partial_membership() {
    let birth = birth(4, workset());
    let before = birth.clone();
    let observed = observation("host", "boot");
    let part = PartId::bind(&birth.body_id, observed.host_id.as_str(), 0).unwrap();
    assert!(BodyLifecycleSession::prepare_first_host(
        &birth,
        part,
        observed,
        SignId::from("same/sign"),
        SignId::from("same/sign"),
    )
    .is_err());
    assert_eq!(birth, before);
}
