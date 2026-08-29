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
        "browser/tour",
        "browser-boot/tour",
        SEED,
        sequence,
        interaction,
    )
    .unwrap()
}

#[test]
fn explicit_birth_retains_one_lulled_body_and_current_here_part() {
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
    assert_eq!(receipt.raw_membership.parts.len(), 1);
    assert_eq!(receipt.raw_membership.events.len(), 2);
    let part = &receipt.raw_membership.parts[0];
    assert_eq!(part.state, MembershipState::Admitted);
    let current = part.current.as_ref().unwrap();
    assert_eq!(current.host_id.as_str(), receipt.host_id);
    assert_eq!(current.boot_id.as_str(), receipt.boot_id);
    assert!(matches!(
        receipt.raw_membership.events[1].kind,
        MembershipEventKind::HostAttached { .. }
    ));
    assert_eq!(session::current().unwrap().body_id, receipt.body_id);
}

#[test]
fn duplicate_birth_and_changed_source_refuse_without_mutating_the_body() {
    session::clear_for_test();
    let first = birth(21);
    let duplicate = admit_source(SEED.as_bytes(), 22).unwrap();
    assert!(
        session::birth("browser/tour", "browser-boot/tour", SEED, 22, duplicate)
            .unwrap_err()
            .contains("duplicate BIRTH")
    );
    assert_eq!(session::current().unwrap().body_id, first.body_id);

    session::clear_for_test();
    let admitted = admit_source(SEED.as_bytes(), 23).unwrap();
    let changed = SEED.replace("hello across", "changed after admission");
    assert!(
        session::birth("browser/tour", "browser-boot/tour", &changed, 23, admitted)
            .unwrap_err()
            .contains("source changed")
    );
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
        malformed,
        31,
        interaction
    )
    .is_err());
    assert!(session::current().is_none());
    assert!(admit_source(b"", 32).is_err());
}
