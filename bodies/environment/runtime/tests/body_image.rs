#![cfg(feature = "body-image")]
use conduit_body::{
    Body, BodyBiographyEvidence, BodyBiographyRecordKind, BodyLifecycleSession, BodyMembership,
    BodyState,
};
use conduit_core::{bind_sign, HostAdvertisement, OfferGeneration};
use conduit_environment_runtime::body_image::{
    open_installed_image, ImageInstallationRefusal as E,
};

fn image(sequence: u64) -> BodyBiographyEvidence {
    let body = Body::born(
        "source/environment".into(),
        "checked/environment".into(),
        sequence,
        "sign/construction".into(),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    BodyBiographyEvidence::born(body, membership, "A retained name".into()).unwrap()
}
fn root(boot: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "host/actual-root".into(),
        boot_id: boot.into(),
        offer_generation: OfferGeneration(7),
        profile: "test/initialized-root".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
#[test]
fn installed_root_joins_existing_body_without_starting_or_changing_workset() {
    let evidence = image(11);
    let initialized = root("boot/current");
    let owner = open_installed_image(evidence.clone(), &initialized).unwrap();
    let retained = owner.evidence();
    retained.validate().unwrap();
    assert_eq!(retained.body, evidence.body);
    assert_eq!(retained.body_id, evidence.body_id);
    assert_eq!(retained.friendly_name, evidence.friendly_name);
    assert_eq!(retained.body.state, BodyState::Lulled);
    assert!(owner.realization().is_none());
    assert_eq!(retained.records.len(), 3);
    assert!(matches!(
        retained.records[1].kind,
        BodyBiographyRecordKind::PartAdmitted { .. }
    ));
    assert!(matches!(
        retained.records[2].kind,
        BodyBiographyRecordKind::HostJoined { .. }
    ));
    for (record, sequence) in retained.records[1..].iter().zip([12, 13]) {
        assert_eq!(record.sequence, sequence);
        assert_eq!(
            record.sign_id,
            bind_sign(&initialized.host_id, &initialized.boot_id, None, sequence).sign_id
        );
    }
    let current = retained.membership.parts[0].current.as_ref().unwrap();
    assert_eq!(current.host_id, initialized.host_id);
    assert_eq!(current.boot_id, initialized.boot_id);
    assert_eq!(current.offer_generation, initialized.offer_generation);
    assert!(BodyLifecycleSession::open_admitted(
        retained.clone(),
        &initialized.host_id,
        &"boot/stale".into()
    )
    .is_err());
}
#[test]
fn existing_membership_cannot_use_first_install_to_replace_a_boot() {
    let owner = open_installed_image(image(1), &root("boot/first")).unwrap();
    assert_eq!(
        open_installed_image(owner.evidence().clone(), &root("boot/second")).unwrap_err(),
        E::NotPristine
    );
}
#[test]
fn current_boot_changes_membership_signs_without_rebirthing_body() {
    let evidence = image(1);
    let first = open_installed_image(evidence.clone(), &root("boot/one")).unwrap();
    let second = open_installed_image(evidence, &root("boot/two")).unwrap();
    assert_eq!(first.evidence().body_id, second.evidence().body_id);
    assert_ne!(
        first.evidence().records[1].sign_id,
        second.evidence().records[1].sign_id
    );
}
#[test]
fn invalid_image_scope_and_sequence_exhaustion_refuse_installation() {
    let mut invalid = image(1);
    invalid.body_id = image(2).body_id;
    assert_eq!(
        open_installed_image(invalid, &root("boot/current")).unwrap_err(),
        E::Biography
    );
    assert_eq!(
        open_installed_image(image(u64::MAX - 1), &root("boot/current")).unwrap_err(),
        E::SequenceExhausted
    );
    let mut absent = root("boot/current");
    absent.offer_generation = OfferGeneration(0);
    assert_eq!(
        open_installed_image(image(1), &absent).unwrap_err(),
        E::HostScope
    );
    absent = root("");
    assert_eq!(
        open_installed_image(image(1), &absent).unwrap_err(),
        E::HostScope
    );
}

#[test]
fn sealed_image_owner_uses_the_same_membership_evidence_as_the_hosted_owner() {
    let evidence = image(7);
    let initialized = root("boot/current");
    let hosted = open_installed_image(evidence.clone(), &initialized).unwrap();
    let retained = conduit_environment_runtime::body_image::open_installed_image_sealed(
        evidence,
        &initialized,
    )
    .unwrap();
    assert_eq!(hosted.evidence(), retained.evidence());
    assert!(retained.realization().is_none());
}

#[test]
fn pristine_compiled_representation_keeps_the_complete_installed_evidence() {
    use conduit_environment_runtime::body_image::restore_pristine_image;
    for sequence in [1, 11, u64::from(u32::MAX)] {
        let original = image(sequence);
        let restored = restore_pristine_image(
            original.body.workset.clone(),
            sequence,
            original.records[0].sign_id.clone(),
            original.friendly_name.clone(),
            original.body_id.as_str(),
        )
        .unwrap();
        assert_eq!(restored, original);
        let initialized = root("boot/representation");
        assert_eq!(
            open_installed_image(restored, &initialized)
                .unwrap()
                .evidence(),
            open_installed_image(original, &initialized)
                .unwrap()
                .evidence()
        );
    }
}
#[test]
fn changed_pristine_seed_cannot_restore_the_expected_body_identity() {
    use conduit_environment_runtime::body_image::{
        restore_pristine_image, PristineImageRefusal as P,
    };
    let original = image(11);
    let mut changed_workset = original.body.workset.clone();
    changed_workset
        .add(conduit_body::ResidentPlot::new(
            "other/source".into(),
            "other/checked".into(),
        ))
        .unwrap();
    assert_eq!(
        restore_pristine_image(
            changed_workset,
            11,
            original.records[0].sign_id.clone(),
            original.friendly_name.clone(),
            original.body_id.as_str()
        ),
        Err(P::Identity)
    );
    let restore = |sequence, sign, name, expected: &str| {
        restore_pristine_image(
            original.body.workset.clone(),
            sequence,
            sign,
            name,
            expected,
        )
    };
    assert_eq!(
        restore(
            12,
            original.records[0].sign_id.clone(),
            original.friendly_name.clone(),
            original.body_id.as_str()
        ),
        Err(P::Identity)
    );
    assert_eq!(
        restore(
            11,
            "other/sign".into(),
            original.friendly_name.clone(),
            original.body_id.as_str()
        ),
        Err(P::Identity)
    );
    assert_eq!(
        restore(
            11,
            original.records[0].sign_id.clone(),
            original.friendly_name.clone(),
            "claimed/body"
        ),
        Err(P::Identity)
    );
    assert_eq!(
        restore(
            11,
            "".into(),
            original.friendly_name.clone(),
            original.body_id.as_str()
        ),
        Err(P::Birth)
    );
    assert_eq!(
        restore(
            11,
            original.records[0].sign_id.clone(),
            "".into(),
            original.body_id.as_str()
        ),
        Err(P::Biography)
    );
}
