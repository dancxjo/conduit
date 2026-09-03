use super::session;
use crate::source_interaction::admit_source;
use conduit_body::{BodyLifecycleEvent, BodyState, MembershipEventKind, MembershipState};

const SEED: &str = r#"form hello_across {
    message: text/literal("hello across one planned Cord")
    show: presentation/text
    message > show
}"#;

const TWO_FORMS: &str = include_str!("../../../../../forms/initial-body.conduit");

fn birth(sequence: u64) -> super::protocol::BirthReceipt {
    let interaction = admit_source(SEED.as_bytes(), sequence).unwrap();
    session::birth(
        "browser/creche",
        "browser-boot/creche",
        "brisk lantern",
        r#"["hello_across"]"#,
        SEED,
        sequence,
        interaction,
    )
    .unwrap()
}

#[test]
fn explicit_birth_retains_one_lulled_body_then_attaches_the_first_host() {
    session::clear_for_test();
    let receipt = birth(11);
    assert_eq!(receipt.disposition, "born");
    assert_eq!(receipt.state, "LULLED");
    assert_eq!(receipt.raw_body.state, BodyState::Lulled);
    assert!(matches!(
        receipt.raw_body.events.as_slice(),
        [BodyLifecycleEvent::Born { .. }]
    ));
    assert!(receipt.wake_id.is_none());
    assert!(receipt.plan_id.is_none());
    assert!(receipt.active_play_id.is_none());
    assert_eq!(receipt.friendly_name, "brisk lantern");
    assert_eq!(receipt.initial_forms.len(), 1);
    assert_eq!(receipt.initial_forms[0].name, "hello_across");
    assert!(receipt.here_part_id.is_none());
    assert!(receipt.raw_membership.parts.is_empty());

    let attached = session::attach_here("browser/creche", "browser-boot/creche", 12).unwrap();
    assert_eq!(attached.raw_membership.parts.len(), 1);
    assert_eq!(attached.raw_membership.events.len(), 2);
    let part = &attached.raw_membership.parts[0];
    assert_eq!(part.state, MembershipState::Admitted);
    let current = part.current.as_ref().unwrap();
    assert_eq!(Some(current.host_id.as_str()), attached.host_id.as_deref());
    assert_eq!(Some(current.boot_id.as_str()), attached.boot_id.as_deref());
    assert!(matches!(
        attached.raw_membership.events[1].kind,
        MembershipEventKind::HostAttached { .. }
    ));
    assert_eq!(session::current().unwrap().body_id, receipt.body_id);
}

#[test]
fn duplicate_birth_and_changed_source_refuse_without_mutating_the_body() {
    session::clear_for_test();
    let first = birth(21);
    let duplicate = admit_source(SEED.as_bytes(), 22).unwrap();
    assert!(session::birth(
        "browser/creche",
        "browser-boot/creche",
        "brisk lantern",
        r#"["hello_across"]"#,
        SEED,
        22,
        duplicate,
    )
    .unwrap_err()
    .contains("duplicate BIRTH"));
    assert_eq!(session::current().unwrap().body_id, first.body_id);

    session::clear_for_test();
    let admitted = admit_source(SEED.as_bytes(), 23).unwrap();
    let changed = SEED.replace("hello across", "changed after admission");
    assert!(session::birth(
        "browser/creche",
        "browser-boot/creche",
        "brisk lantern",
        r#"["hello_across"]"#,
        &changed,
        23,
        admitted,
    )
    .unwrap_err()
    .contains("source changed"));
    assert!(session::current().is_none());
}

#[test]
fn birth_activates_multiple_selected_forms_as_one_revision_zero_workload() {
    session::clear_for_test();
    let interaction = admit_source(TWO_FORMS.as_bytes(), 24).unwrap();
    let receipt = session::birth(
        "browser/creche",
        "browser-boot/creche",
        "shared lantern",
        r#"["memory_lantern","morse_network"]"#,
        TWO_FORMS,
        24,
        interaction,
    )
    .unwrap();
    assert_eq!(receipt.initial_forms.len(), 2);
    assert_eq!(receipt.raw_body.workload_revision, 0);
    assert_eq!(receipt.raw_body.workset.len(), 2);
    assert!(receipt.raw_body.validate().is_ok());
}

#[test]
fn duplicate_absent_and_over_capacity_initial_selections_refuse_before_birth() {
    for (sequence, selection) in [
        (25, r#"["hello_across","hello_across"]"#.to_string()),
        (26, r#"["absent"]"#.to_string()),
        (
            27,
            serde_json::to_string(&vec!["hello_across"; conduit_body::MAX_BODY_FORMS + 1]).unwrap(),
        ),
    ] {
        session::clear_for_test();
        let interaction = admit_source(SEED.as_bytes(), sequence).unwrap();
        assert!(session::birth(
            "browser/creche",
            "browser-boot/creche",
            "bounded refusal",
            &selection,
            SEED,
            sequence,
            interaction,
        )
        .is_err());
        assert!(session::current().is_none());
    }
}

#[test]
fn malformed_or_empty_form_source_is_refused_before_body_creation() {
    session::clear_for_test();
    let malformed = "form broken { nope: missing/kind }";
    let interaction = admit_source(malformed.as_bytes(), 31).unwrap();
    assert!(session::birth(
        "browser/tour",
        "browser-boot/tour",
        "brisk lantern",
        r#"["hello_across"]"#,
        malformed,
        31,
        interaction
    )
    .is_err());
    assert!(session::current().is_none());
    assert!(admit_source(b"", 32).is_err());
}

#[test]
fn friendly_name_is_metadata_and_cannot_change_durable_body_identity() {
    session::clear_for_test();
    let first_interaction = admit_source(SEED.as_bytes(), 41).unwrap();
    let first = session::birth(
        "browser/creche",
        "browser-boot/creche",
        "patient firefly",
        r#"["hello_across"]"#,
        SEED,
        41,
        first_interaction,
    )
    .unwrap();
    session::clear_for_test();
    let second_interaction = admit_source(SEED.as_bytes(), 41).unwrap();
    let second = session::birth(
        "browser/creche",
        "browser-boot/creche",
        "steady willow",
        r#"["hello_across"]"#,
        SEED,
        41,
        second_interaction,
    )
    .unwrap();
    assert_eq!(first.body_id, second.body_id);
    assert_ne!(first.friendly_name, second.friendly_name);

    session::clear_for_test();
    let unsupported = admit_source(SEED.as_bytes(), 42).unwrap();
    assert!(session::birth(
        "browser/creche",
        "browser-boot/creche",
        "steady willow",
        r#"["absent_form"]"#,
        SEED,
        42,
        unsupported,
    )
    .unwrap_err()
    .contains("absent from checked source"));
}

#[test]
fn graduation_requires_a_current_part_and_preserves_body_identity_for_both_choices() {
    session::clear_for_test();
    let born = birth(51);
    assert!(!super::graduation::readiness().unwrap().ready);
    session::attach_here("browser/creche", "browser-boot/creche", 52).unwrap();
    assert!(super::graduation::readiness().unwrap().ready);
    let hosted = super::graduation::graduate(1, 54).unwrap();
    assert_eq!(hosted.body_id, born.body_id);
    let evidence = hosted.graduation.unwrap();
    assert_eq!(evidence.choice, "host-patchbay");
    assert!(evidence.patchbay_plan_id.is_some());
    assert_eq!(
        evidence.patchbay_implementation_id.as_deref(),
        Some("browser/patchbay-surface@1")
    );
    assert!(!evidence.creche_required);
    let biography = session::biography().unwrap();
    biography.validate().unwrap();
    assert_eq!(biography.body_id.as_str(), born.body_id);
    assert_eq!(biography.records.len(), 4);
    assert!(matches!(
        biography.records.last().unwrap().kind,
        conduit_body::BodyBiographyRecordKind::Graduated {
            choice: conduit_body::BodyGraduationChoice::HostedPatchbay,
            ..
        }
    ));
    assert!(super::graduation::graduate(2, 55)
        .unwrap_err()
        .contains("already graduated"));

    session::clear_for_test();
    let born = birth(61);
    session::attach_here("browser/creche", "browser-boot/creche", 62).unwrap();
    let external = super::graduation::graduate(2, 64).unwrap();
    assert_eq!(external.body_id, born.body_id);
    let evidence = external.graduation.unwrap();
    assert_eq!(evidence.choice, "external-reader");
    assert!(evidence.patchbay_plan_id.is_none());
    let biography = session::biography().unwrap();
    let reopened: conduit_body::BodyBiographyEvidence =
        serde_json::from_str(&serde_json::to_string(&biography).unwrap()).unwrap();
    reopened.validate().unwrap();
    assert_eq!(reopened.body_id.as_str(), born.body_id);
    assert!(matches!(
        reopened.records.last().unwrap().kind,
        conduit_body::BodyBiographyRecordKind::Graduated {
            choice: conduit_body::BodyGraduationChoice::ExternalReader,
            patchbay_plan_id: None,
            patchbay_implementation_id: None,
        }
    ));
}

#[test]
fn durable_snapshot_restores_exact_validated_body_truth_but_not_transient_work() {
    session::clear_for_test();
    let born = birth(71);
    session::attach_here("browser/creche", "browser-boot/creche", 72).unwrap();
    let graduated = super::graduation::graduate(2, 74).unwrap();
    let snapshot = session::durable_snapshot().unwrap();
    let encoded = serde_json::to_vec(&snapshot).unwrap();
    assert!(encoded.len() <= 32 * 1_024);

    session::clear_for_test();
    let decoded = serde_json::from_slice(&encoded).unwrap();
    let restored = session::restore_durable(decoded).unwrap();
    assert_eq!(restored, graduated);
    assert_eq!(restored.body_id, born.body_id);
    assert_eq!(session::biography().unwrap(), snapshot.biography);
    assert!(super::graduation::readiness().unwrap().ready);

    assert!(session::restore_durable(snapshot)
        .unwrap_err()
        .contains("already has a Body"));
}

#[test]
fn changed_durable_identity_refuses_atomically() {
    session::clear_for_test();
    birth(81);
    let mut changed = session::durable_snapshot().unwrap();
    changed.receipt.body_id.push_str("-changed");
    session::clear_for_test();
    assert!(session::restore_durable(changed)
        .unwrap_err()
        .contains("identities disagree"));
    assert!(session::current().is_none());
}

#[test]
fn durable_browser_host_reboots_through_exact_detach_and_attach_events() {
    session::clear_for_test();
    let born = birth(91);
    session::attach_here("browser/creche", "browser-boot/one", 92).unwrap();
    let snapshot = session::durable_snapshot().unwrap();
    session::clear_for_test();
    session::restore_durable(snapshot).unwrap();

    let reconciled = session::attach_here("browser/creche", "browser-boot/two", 94).unwrap();
    assert_eq!(reconciled.body_id, born.body_id);
    assert_eq!(reconciled.membership_revision, 4);
    assert_eq!(reconciled.boot_id.as_deref(), Some("browser-boot/two"));
    assert!(matches!(
        reconciled.raw_membership.events[2].kind,
        MembershipEventKind::HostDetached { .. }
    ));
    assert!(matches!(
        reconciled.raw_membership.events[3].kind,
        MembershipEventKind::HostAttached { .. }
    ));
    assert_eq!(
        reconciled.raw_membership.parts[0]
            .current
            .as_ref()
            .unwrap()
            .sequence,
        2
    );
    session::biography().unwrap().validate().unwrap();
}

#[test]
fn host_mismatch_refuses_while_leave_rejoin_revoke_and_forget_remain_distinct() {
    session::clear_for_test();
    let born = birth(101);
    session::attach_here("browser/creche", "browser-boot/one", 102).unwrap();
    let before = session::current().unwrap();
    assert!(
        session::attach_here("browser/creche", "browser-boot/stale-sequence", 1)
            .unwrap_err()
            .contains("biography")
    );
    assert_eq!(session::current().unwrap(), before);
    assert!(
        session::attach_here("browser/other", "browser-boot/two", 104)
            .unwrap_err()
            .contains("does not match")
    );
    assert_eq!(session::current().unwrap(), before);

    let left = session::leave_here("browser/creche", "browser-boot/one", 105).unwrap();
    assert_eq!(left.body_id, born.body_id);
    assert!(left.boot_id.is_none());
    assert_eq!(
        left.raw_membership.parts[0].state,
        MembershipState::Admitted
    );
    assert!(left.raw_membership.parts[0].current.is_none());

    let returned = session::attach_here("browser/creche", "browser-boot/two", 106).unwrap();
    assert_eq!(
        returned.raw_membership.parts[0].state,
        MembershipState::Admitted
    );
    assert_eq!(returned.boot_id.as_deref(), Some("browser-boot/two"));
    let revoked = session::revoke_here("browser/creche", "browser-boot/two", 107).unwrap();
    assert_eq!(revoked.body_id, born.body_id);
    assert_eq!(
        revoked.raw_membership.parts[0].state,
        MembershipState::Revoked
    );
    assert!(revoked.raw_membership.parts[0].current.is_none());

    let durable_evidence = session::durable_snapshot().unwrap();
    session::forget_local();
    assert!(session::current().is_none());
    assert_eq!(durable_evidence.receipt.body_id, born.body_id);
}
