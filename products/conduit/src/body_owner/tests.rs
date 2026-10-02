use super::*;
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::StdHostConfig;
const SOURCE: &str = "plot hello {\n show: presentation/text\n \"Hello.\" >> show\n}.";
fn source() -> conduit_plot::ExpandedAuthoringPlot {
    crate::plot_source::parse(SOURCE)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap()
}
fn host(boot: &str) -> StdHost {
    StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("host/owner-test"),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
    })
}
fn resident(plot: &conduit_plot::ExpandedAuthoringPlot) -> ResidentPlot {
    ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    )
}
#[test]
fn checked_birth_and_fresh_boot_recovery_preserve_body_without_replaying_proposal() {
    let plot = source();
    let mut owner = Owner::open(host("boot/first"), resident(&plot), None, "Test Body").unwrap();
    let id = owner.session.evidence().body_id.clone();
    assert_eq!(
        owner.session.evidence().body.state,
        conduit_body::BodyState::Lulled
    );
    assert!(owner.session.realization().is_none());
    owner.plan(&plot).unwrap();
    assert!(owner.session.realization().unwrap().play.is_none());
    let recovered = Owner::open(
        host("boot/second"),
        resident(&plot),
        Some(owner.session.evidence().clone()),
        "ignored on recovery",
    )
    .unwrap();
    assert_eq!(recovered.session.evidence().body_id, id);
    assert_eq!(recovered.session.evidence().friendly_name, "Test Body");
    assert_eq!(
        recovered.session.evidence().body.state,
        conduit_body::BodyState::Lulled
    );
    assert!(recovered.session.realization().is_none());
    assert!(recovered
        .session
        .evidence()
        .wakes
        .iter()
        .any(|wake| wake.lifecycle == conduit_body::WakeLifecycle::Failed));
}
#[test]
fn recovery_refuses_same_boot_wrong_source_and_corrupt_biography() {
    let plot = source();
    let owner = Owner::open(host("boot/first"), resident(&plot), None, "Test Body").unwrap();
    let evidence = owner.session.evidence().clone();
    assert!(Owner::open(
        host("boot/first"),
        resident(&plot),
        Some(evidence.clone()),
        "Test"
    )
    .is_err());
    let wrong = ResidentPlot::new("source/wrong".into(), "checked/wrong".into());
    assert!(Owner::open(host("boot/next"), wrong, Some(evidence.clone()), "Test").is_err());
    let mut corrupt = evidence;
    corrupt.schema = "wrong".into();
    assert!(Owner::open(host("boot/next"), resident(&plot), Some(corrupt), "Test").is_err());
}
#[test]
fn ordinary_kernel_execution_supplies_actual_play_and_output_then_retains_lull() {
    let plot = source();
    let mut owner = Owner::open(host("boot/run"), resident(&plot), None, "Test Body").unwrap();
    assert!(owner.execute(100).is_err());
    owner.plan(&plot).unwrap();
    owner.execute(1000).unwrap();
    let execution = owner.last_execution.as_ref().unwrap();
    assert_eq!(
        execution["terminal"],
        serde_json::to_value(conduit_core::TerminalDisposition::Completed).unwrap()
    );
    assert!(execution["output_utf8"]
        .as_str()
        .unwrap()
        .contains("Hello."));
    assert!(execution["play"]["active_play_id"].is_string());
    assert!(execution["cleanup_failure"].is_null());
    assert!(owner.session.realization().is_none());
    assert_eq!(
        owner.session.evidence().body.state,
        conduit_body::BodyState::Lulled
    );
}
#[test]
fn independent_births_on_same_host_and_source_get_distinct_bodies() {
    let plot = source();
    let first = Owner::open(host("boot/a"), resident(&plot), None, "First").unwrap();
    let second = Owner::open(host("boot/b"), resident(&plot), None, "Second").unwrap();
    assert_ne!(
        first.session.evidence().body_id,
        second.session.evidence().body_id
    );
}
#[test]
fn operating_system_lock_excludes_another_owner_and_releases_on_drop() {
    let root = std::env::temp_dir().join(format!("conduit-owner-lock-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let first = super::super::super::owner_lock::acquire(&root).unwrap();
    assert!(super::super::super::owner_lock::acquire(&root).is_err());
    drop(first);
    let next = super::super::super::owner_lock::acquire(&root).unwrap();
    drop(next);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn subsequent_runs_keep_body_and_create_distinct_admitted_plays() {
    let plot = source();
    let mut owner = Owner::open(host("boot/repeated"), resident(&plot), None, "Test Body").unwrap();
    let body = owner.session.evidence().body_id.clone();
    owner.plan(&plot).unwrap();
    owner.execute(1000).unwrap();
    let first = owner.last_execution.as_ref().unwrap()["play"]["active_play_id"].clone();
    owner.plan(&plot).unwrap();
    owner.execute(1000).unwrap();
    assert_eq!(owner.session.evidence().body_id, body);
    assert_ne!(
        owner.last_execution.as_ref().unwrap()["play"]["active_play_id"],
        first
    );
}

#[test]
fn unsupported_advertised_call_retains_untyped_refusal_without_inventing_play() {
    let plot = crate::plot_source::parse("plot refused {\n upper: text/upper\n show: presentation/text\n \"Hello.\" >> upper >> show\n}.")
        .unwrap().expand_entry_for_authoring().unwrap();
    let mut owner = Owner::open(host("boot/refused"), resident(&plot), None, "Refused").unwrap();
    owner.plan(&plot).unwrap();
    let proposal = owner.session.realization().unwrap().plan.plan_id.clone();
    assert!(owner
        .execute(1000)
        .unwrap_err()
        .contains("Body Host Call is unsupported"));
    let receipt = owner.last_execution.as_ref().unwrap();
    assert_eq!(receipt["plan_id"], serde_json::to_value(proposal).unwrap());
    assert!(receipt["play"].is_null());
    assert_eq!(receipt["refusal_scope"], "untyped-host-admission");
    assert!(owner.session.realization().unwrap().play.is_none());
}

#[test]
fn actual_execution_receipt_survives_fresh_boot_as_history_only() {
    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "owner-receipt-test",
        "recovery",
    ));
    std::fs::create_dir_all(&root).unwrap();
    let installation = super::super::super::Installation {
        schema: super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/owner-test".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: super::super::super::digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
    };
    super::super::super::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let plot = source();
    let mut owner = Owner::open(
        host("boot/receipt-first"),
        resident(&plot),
        None,
        "Retained",
    )
    .unwrap();
    owner.plan(&plot).unwrap();
    owner.execute(1000).unwrap();
    owner.persist(&root).unwrap();
    let historical = owner.last_execution.clone();
    let mut reopened = Owner::open(
        host("boot/receipt-next"),
        resident(&plot),
        state::load(&root).unwrap(),
        "ignored",
    )
    .unwrap();
    reopened.restore_execution(&root).unwrap();
    assert_eq!(reopened.last_execution, historical);
    assert!(reopened.session.realization().is_none());
    assert_eq!(
        reopened.last_execution.as_ref().unwrap()["boot_id"],
        "boot/receipt-first"
    );
    std::fs::write(root.join("body/owner-execution.json"), b"{}").unwrap();
    assert!(reopened.restore_execution(&root).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
