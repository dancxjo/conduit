use super::session;
use crate::book_runner::interaction::admit_source;
use conduit_body::{BodyLifecycleEvent, BodyState, MembershipEventKind, MembershipState};

const SEED: &str = r#"form hello_across {
    message: text/literal("hello across one planned Cord")
    show: presentation/text
    message > show
}"#;

fn birth(sequence: u64) -> super::protocol::BirthReceipt {
    let interaction = admit_source(SEED.as_bytes(), sequence).unwrap();
    session::birth(
        "browser/creche",
        "browser-boot/creche",
        "brisk lantern",
        "morse-network@1",
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
    assert_eq!(receipt.initial_program, "morse-network@1");
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
        "morse-network@1",
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
        "morse-network@1",
        &changed,
        23,
        admitted,
    )
    .unwrap_err()
    .contains("source changed"));
    assert!(session::current().is_none());
}

#[test]
fn malformed_or_empty_seed_is_refused_before_body_creation() {
    session::clear_for_test();
    let malformed = "form broken { nope: missing/kind }";
    let interaction = admit_source(malformed.as_bytes(), 31).unwrap();
    assert!(session::birth(
        "browser/tour",
        "browser-boot/tour",
        "brisk lantern",
        "morse-network@1",
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
        "morse-network@1",
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
        "morse-network@1",
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
        "unreviewed-program@1",
        SEED,
        42,
        unsupported,
    )
    .unwrap_err()
    .contains("reviewed Morse Network"));
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
}
