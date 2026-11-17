//! Actual prepared member terminal through real owner persistence, injected ACK failures.
use super::*;
use crate::durable_host_control::startup_member::{completed_custody, TerminalControl};
use std::fs;
fn fixture() -> (
    std::path::PathBuf,
    Owner,
    DelegatedBodyTerminalReceipt,
    TerminalControl,
) {
    let (host, session, terminal, control) = completed_custody();
    let root = control
        .joined
        .root
        .parent()
        .unwrap()
        .join("owner-terminal-custody");
    fs::create_dir(&root).unwrap();
    let mut installation: crate::durable_host::Installation =
        serde_json::from_slice(&fs::read(control.joined.root.join("installation.json")).unwrap())
            .unwrap();
    installation.host_id = host.advertisement().host_id.as_str().into();
    installation.joined_body_state = None;
    installation.body_state = None;
    fs::write(
        root.join("installation.json"),
        serde_json::to_vec(&installation).unwrap(),
    )
    .unwrap();
    let resident = session.evidence().body.workset.plots()[0].clone();
    let mut owner = Owner::open(host, resident, None, "Actual terminal custody").unwrap();
    owner.session = session;
    owner.persist(&root).unwrap();
    (root, owner, terminal, control)
}
fn assert_playing(root: &Path, owner: &Owner, terminal: &DelegatedBodyTerminalReceipt) {
    assert_eq!(
        owner.session.realization().unwrap().play.as_ref(),
        Some(&terminal.global_play)
    );
    let retained = super::super::super::state::load(root).unwrap().unwrap();
    assert!(matches!(
        retained.body.state,
        conduit_body::BodyState::Awake { .. }
    ));
    assert!(retained
        .wakes
        .iter()
        .flat_map(|wake| &wake.plans)
        .any(|plan| plan.state == conduit_body::WakePlanState::Playing
            && plan.active_play_id.as_ref() == Some(&terminal.global_play.active_play_id)));
}
#[test]
fn cleanup_uncertainty_retains_original_play_and_does_not_attempt_member_release() {
    let (root, mut owner, mut terminal, mut control) = fixture();
    // A received cleanup uncertainty is a refusal input, never fabricated success.
    terminal.cleanup_failure = Some("injected remote cleanup uncertainty".into());
    let called = std::cell::Cell::new(false);
    assert!(!finish(&mut owner, &root, &terminal, || {
        called.set(true);
        control.release()
    })
    .unwrap());
    assert!(!called.get());
    assert!(control.retains_ticket());
    assert_playing(&root, &owner, &terminal);
    assert_eq!(
        owner.last_execution.as_ref().unwrap()["member_release"],
        "cleanup-unknown"
    );
    control.release().unwrap();
}
#[test]
fn lost_actual_release_ack_retains_durable_play_without_implicit_retry() {
    let (root, mut owner, terminal, mut control) = fixture();
    assert!(!finish(&mut owner, &root, &terminal, || {
        control.release()?;
        Err("injected lost release acknowledgment".into())
    })
    .unwrap());
    assert!(!control.retains_ticket());
    assert_playing(&root, &owner, &terminal);
    assert_eq!(
        owner.last_execution.as_ref().unwrap()["member_release"],
        "unknown"
    );
}
#[test]
fn positive_release_precedes_lull_and_failed_final_save_preserves_playing() {
    let (root, mut owner, terminal, mut control) = fixture();
    let held = root.join("body-held");
    let result = finish(&mut owner, &root, &terminal, || {
        control.release()?;
        fs::rename(root.join("body"), &held).unwrap();
        fs::write(root.join("body"), b"block final transaction").unwrap();
        Ok(())
    });
    assert!(result.is_err());
    assert!(!control.retains_ticket());
    fs::remove_file(root.join("body")).unwrap();
    fs::rename(&held, root.join("body")).unwrap();
    assert_playing(&root, &owner, &terminal);
    assert_eq!(
        owner.last_execution.as_ref().unwrap()["member_release"],
        "released"
    );
}
#[test]
fn acknowledged_actual_release_allows_original_owner_to_persist_lull() {
    let (root, mut owner, terminal, mut control) = fixture();
    assert!(finish(&mut owner, &root, &terminal, || control.release()).unwrap());
    assert!(owner.session.realization().is_none());
    assert!(!control.retains_ticket());
    assert_eq!(
        super::super::super::state::load(&root)
            .unwrap()
            .unwrap()
            .body
            .state,
        conduit_body::BodyState::Lulled
    );
}
