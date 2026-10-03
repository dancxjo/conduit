use super::*;
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::StdHostConfig;
const SOURCE: &str = "plot hello {\n show: presentation/text\n \"Hello.\" >> show\n}.";
const CLOCK_SOURCE: &str = include_str!("../../../../plots/clock/main.conduit");
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
fn service_clock_runs_with_durable_live_play_and_explicit_lull() {
    use std::time::{Duration, Instant};

    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "owner-clock-service-test",
        "continuing",
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
    let plot = crate::plot_source::parse(CLOCK_SOURCE)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let mut owner =
        Owner::open(host("boot/clock-service"), resident(&plot), None, "Clock").unwrap();
    owner.persist(&root).unwrap();
    std::fs::write(root.join("body/source.conduit"), CLOCK_SOURCE).unwrap();

    let mut worker = owner.start_service_run(&root, 5000).unwrap();
    assert!(owner.host.is_playing());
    assert!(owner.start_service_run(&root, 5000).is_err());
    let start = Instant::now();
    while owner.current_play_id().is_none() {
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "clock Play did not start"
        );
        assert!(!worker.progress(&mut owner, &root).unwrap());
        std::thread::sleep(Duration::from_millis(10));
    }
    let play = owner.current_play_id().unwrap().clone();
    assert_eq!(
        owner.truth()["realization"]["play"]["active_play_id"],
        play.as_str()
    );
    assert!(matches!(
        state::load(&root).unwrap().unwrap().body.state,
        conduit_body::BodyState::Awake { .. }
    ));
    std::thread::sleep(Duration::from_millis(2200));
    worker.request_lull().unwrap();
    assert!(worker.request_lull().is_err());
    let stopping = Instant::now();
    while !worker.progress(&mut owner, &root).unwrap() {
        assert!(
            stopping.elapsed() < Duration::from_secs(3),
            "clock Play did not stop"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!owner.host.is_playing());
    assert!(owner.current_play_id().is_none());
    assert_eq!(
        state::load(&root).unwrap().unwrap().body.state,
        conduit_body::BodyState::Lulled
    );
    let receipt = owner.last_execution.as_ref().unwrap();
    assert_eq!(receipt["play"]["active_play_id"], play.as_str());
    assert_eq!(
        receipt["terminal"],
        serde_json::to_value(conduit_core::TerminalDisposition::Cancelled {
            reason: conduit_core::CancellationReason::OperatorRequested,
        })
        .unwrap()
    );
    assert!(
        receipt["output_utf8"]
            .as_str()
            .unwrap()
            .contains("tick sequence="),
        "{receipt:?}"
    );

    std::fs::write(root.join("body/source.conduit"), SOURCE).unwrap();
    assert!(owner.start_service_run(&root, 5000).is_err());
    assert!(owner.session.realization().is_none());
    std::fs::remove_dir_all(root).unwrap();
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

#[test]
fn retained_invitation_admits_one_native_host_once_in_running_owner() {
    use conduit_body::{
        AdmissionManager, PortableSpawnAdmissionRequest, SpawnInvitationSecret,
        SPAWN_ADMISSION_REQUEST_SCHEMA,
    };
    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "owner-native-admission",
        "invited",
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
    let mut first = Owner::open(host("boot/first"), resident(&plot), None, "Shared Body").unwrap();
    first.persist(&root).unwrap();
    let owner_host = first.host.advertisement();
    let now = super::super::super::current_time_millis().unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes([13; 32]).unwrap();
    let mut manager = AdmissionManager::new(first.session.evidence().body_id.clone()).unwrap();
    let claim = first
        .session
        .issue_invitation(
            &mut manager,
            secret.clone(),
            [17; 32],
            now,
            now + 60_000,
            &owner_host.host_id,
            &owner_host.boot_id,
        )
        .unwrap();
    // The installed invitation entrance writes this canonical file while the
    // owner is stopped; the fresh owner must restore and consume it.
    super::super::super::write_json_atomic(&root.join("body/admission.json"), &manager).unwrap();
    let mut owner = Owner::open(
        host("boot/second"),
        resident(&plot),
        state::load(&root).unwrap(),
        "ignored",
    )
    .unwrap();
    owner.restore_execution(&root).unwrap();
    assert_eq!(owner.admissions, Some(manager));
    let guest = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("host/native-guest"),
        boot_id: BootId::from("boot/native-guest/1"),
        offer_generation: OfferGeneration(1),
    });
    let advertisement = guest.advertisement().clone();
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_advertisement: advertisement.clone(),
        nonce: claim.nonce,
        signature: secret
            .sign(&claim.signing_transcript(
                &advertisement.host_id,
                &advertisement.boot_id,
                advertisement.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    assert!(owner
        .admit_invited(&root, request.clone(), "host/wrong-guest")
        .is_err());
    let mut invalid = request.clone();
    invalid.host_advertisement.protocol_version = 0;
    assert!(owner
        .admit_invited(&root, invalid, "host/native-guest")
        .is_err());
    let receipt = owner
        .admit_invited(&root, request.clone(), "host/native-guest")
        .unwrap();
    receipt.validate_against(&request).unwrap();
    assert_eq!(receipt.credential.host_id, advertisement.host_id);
    assert_eq!(receipt.credential.boot_id, advertisement.boot_id);
    let retained = state::load(&root).unwrap().unwrap();
    assert_eq!(retained.body_id, claim.body_id);
    assert_eq!(retained.membership.parts.len(), 2);
    let before = owner.session.evidence().clone();
    assert!(matches!(
        owner.admit_invited(&root, request.clone(), "host/native-guest"),
        Err(error) if error.contains("Replay")
    ));
    let mut next_manager = owner.admissions.take().unwrap();
    let second_secret = SpawnInvitationSecret::from_csprng_bytes([23; 32]).unwrap();
    let next = owner
        .session
        .issue_invitation(
            &mut next_manager,
            second_secret.clone(),
            [27; 32],
            now,
            now + 60_000,
            &owner.host.advertisement().host_id,
            &owner.host.advertisement().boot_id,
        )
        .unwrap();
    owner.admissions = Some(next_manager);
    let mut duplicate = request;
    duplicate.invitation_id = next.invitation_id.clone();
    duplicate.nonce = next.nonce;
    duplicate.signature = second_secret
        .sign(&next.signing_transcript(
            &advertisement.host_id,
            &advertisement.boot_id,
            advertisement.offer_generation,
        ))
        .to_vec();
    assert!(matches!(
        owner.admit_invited(&root, duplicate, "host/native-guest"),
        Err(error) if error.contains("already has an admitted Part")
    ));
    assert_eq!(owner.session.evidence(), &before);
    std::fs::remove_dir_all(root).unwrap();
}
