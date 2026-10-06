use super::*;
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_presentation::PresentationRole;
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
fn missing_owner_route_witness_keeps_the_sealed_plan_and_withdraws_availability() {
    use conduit_presentation::{CurrentOwnerPresentationRoute, MaskShowDisposition};

    let checked = source();
    let mut owner = Owner::open(
        host("boot/wardrobe-loss"),
        resident(&checked),
        None,
        "Wardrobe",
    )
    .unwrap();
    let (attached, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
    owner.host.attach_terminal_mask(attached).unwrap();
    let (face, seal) = owner.seal_attached_terminal_route().unwrap();
    owner.attached_terminal_route = Some(seal.clone());
    assert!(Owner::current_attached_terminal_route(
        &owner.host,
        owner.attached_terminal_route.as_ref(),
        &owner.session,
        &face,
    )
    .unwrap()
    .is_some());
    let next_face = conduit_presentation::Presentation::new(
        face.revision + 1,
        face.basis.clone(),
        face.subjects.clone(),
        face.relationships.clone(),
        face.properties.clone(),
        face.text.clone(),
    )
    .unwrap();
    assert!(Owner::current_attached_terminal_route(
        &owner.host,
        owner.attached_terminal_route.as_ref(),
        &owner.session,
        &next_face,
    )
    .unwrap()
    .is_none());
    let current = [CurrentOwnerPresentationRoute::Local {
        seal: &seal,
        owner_offer: owner.host.advertisement(),
    }];
    let mask = seal.planned_mask.mask.plot_identity.clone();
    let mut wardrobe = presentation_wardrobe::OwnerPresentationWardrobe::seal(
        &owner.session,
        &face,
        &current,
        vec![mask.clone()],
        vec![mask],
    )
    .unwrap();
    let plan_id = wardrobe.plan().plan_id.clone();
    let lost = wardrobe
        .admit_or_replace(&owner.session, &face, &[])
        .unwrap();
    assert_eq!(wardrobe.plan().plan_id, plan_id);
    assert!(matches!(
        lost.show,
        MaskShowDisposition::NoCurrentShow { .. }
    ));
    assert!(
        !wardrobe
            .plan()
            .admit_current_routes(&owner.session, &face, &[])
            .unwrap()
            .routes()[0]
            .currently_available
    );
    let restored = wardrobe
        .admit_or_replace(&owner.session, &face, &current)
        .unwrap();
    assert_eq!(wardrobe.plan().plan_id, plan_id);
    assert!(matches!(
        restored.show,
        MaskShowDisposition::SelectSealed { .. }
    ));
    let mut stale_offer = owner.host.advertisement().clone();
    stale_offer.offer_generation.0 += 1;
    let stale = [CurrentOwnerPresentationRoute::Local {
        seal: &seal,
        owner_offer: &stale_offer,
    }];
    assert!(wardrobe
        .admit_or_replace(&owner.session, &face, &stale)
        .is_err());
    assert_eq!(wardrobe.plan().plan_id, plan_id);
}

#[test]
fn owner_face_uses_checked_names_at_birth_and_after_fresh_boot() {
    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "owner-face-names",
        "retained-source",
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
        selected_speech: None,
        selected_model: None,
    };
    super::super::super::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let checked = source();
    let mut born = Owner::open(
        host("boot/name-first"),
        resident(&checked),
        None,
        "North Station",
    )
    .unwrap();
    born.set_resident_plot_name(&checked).unwrap();
    born.persist(&root).unwrap();
    let names = |owner: &Owner| {
        let face = owner.local_face_snapshot().unwrap();
        let body = face
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Body)
            .unwrap()
            .name
            .clone();
        let plot = face
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Plot)
            .unwrap()
            .name
            .clone();
        (body, plot, face.basis.body_id)
    };
    let original = names(&born);
    assert_eq!(original.0, "North Station");
    assert_eq!(original.1, checked.expanded.name);
    std::fs::write(root.join("body/source.conduit"), SOURCE).unwrap();
    let resumed = super::super::resume_service(host("boot/name-next"), &root).unwrap();
    assert_eq!(names(&resumed), original);

    // A legacy or interrupted source write preserves the biography and exact
    // Plot identity, but cannot claim a checked human Plot name.
    std::fs::remove_file(root.join("body/source.conduit")).unwrap();
    let legacy = super::super::resume_service(host("boot/name-legacy"), &root).unwrap();
    let legacy_names = names(&legacy);
    assert_eq!(legacy_names.0, "North Station");
    assert_eq!(legacy_names.2, original.2);
    assert!(legacy_names.1.starts_with("Resident Plot "));

    std::fs::write(root.join("body/source.conduit"), CLOCK_SOURCE).unwrap();
    assert!(super::super::resume_service(host("boot/name-wrong"), &root).is_err());
    std::fs::remove_dir_all(root).unwrap();
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
        selected_speech: None,
        selected_model: None,
    };
    super::super::super::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let plot = crate::plot_source::parse(CLOCK_SOURCE)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let mut owner =
        Owner::open(host("boot/clock-service"), resident(&plot), None, "Clock").unwrap();
    owner.set_resident_plot_name(&plot).unwrap();
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
    let playing_face = owner.local_face_snapshot().unwrap();
    let stop = playing_face
        .actions
        .iter()
        .find(|action| action.intent == super::clock_interval::CLOCK_LULL_ACTION)
        .unwrap();
    assert_eq!(
        stop.availability,
        conduit_presentation::PresentationActionAvailability::Available
    );
    let response = conduit_presentation::OwnerFaceSnapshotResponse::Snapshot {
        schema: conduit_presentation::OWNER_FACE_RESPONSE_SCHEMA.into(),
        presentation: Box::new(playing_face),
        interactions_admitted: true,
        route: None,
    };
    let face_bytes = serde_json::to_vec(&response).unwrap().len();
    assert!(
        face_bytes > 8_178,
        "playing Face is only {face_bytes} bytes"
    );
    assert!(
        face_bytes <= conduit_presentation::MAX_OWNER_FACE_RESPONSE_BYTES,
        "playing Face needs {face_bytes} bytes"
    );
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
fn lulled_clock_interval_replaces_checked_workset_and_next_plan_without_rebirth() {
    use std::time::{Duration, Instant};

    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "owner-clock-interval-test",
        "checked-replacement",
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
        selected_speech: None,
        selected_model: None,
    };
    super::super::super::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let initial = crate::plot_source::parse(CLOCK_SOURCE)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let initial_resident = resident(&initial);
    let mut owner = Owner::open(
        host("boot/clock-interval"),
        initial_resident.clone(),
        None,
        "Clock",
    )
    .unwrap();
    owner.persist(&root).unwrap();
    std::fs::write(root.join("body/source.conduit"), CLOCK_SOURCE).unwrap();
    let body_id = owner.session.evidence().body_id.clone();
    let original_plan = owner
        .plan_partition(&initial, &initial_resident)
        .unwrap()
        .plan
        .plan_id;
    assert!(owner.replace_lulled_clock_interval(&root, 1_000).is_err());
    assert!(owner.replace_lulled_clock_interval(&root, 42).is_err());
    owner.replace_lulled_clock_interval(&root, 500).unwrap();
    let replacement = super::super::checked_retained_source(&root)
        .unwrap()
        .unwrap();
    let replacement_resident = resident(&replacement);
    assert_eq!(owner.session.evidence().body_id, body_id);
    assert_eq!(owner.session.evidence().body.workload_revision, 2);
    assert_eq!(
        owner.session.evidence().body.workset.plots(),
        std::slice::from_ref(&replacement_resident)
    );
    assert_ne!(replacement_resident, initial_resident);
    assert_ne!(
        owner
            .plan_partition(&replacement, &replacement_resident)
            .unwrap()
            .plan
            .plan_id,
        original_plan
    );
    assert_eq!(
        state::load(&root).unwrap().unwrap(),
        *owner.session.evidence()
    );
    let mut worker = owner.start_service_run(&root, 5_000).unwrap();
    let started = Instant::now();
    while owner.current_play_id().is_none() {
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(!worker.progress(&mut owner, &root).unwrap());
        std::thread::sleep(Duration::from_millis(10));
    }
    let current = owner.session.realization().unwrap();
    assert_eq!(current.plan.plots[0].plot, replacement_resident);
    assert_ne!(current.plan.plots[0].plan.plan_id, original_plan);
    assert!(owner.replace_lulled_clock_interval(&root, 250).is_err());
    worker.request_lull().unwrap();
    let stopped = Instant::now();
    while !worker.progress(&mut owner, &root).unwrap() {
        assert!(stopped.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(owner.session.evidence().body_id, body_id);
    assert_eq!(
        owner.session.evidence().body.state,
        conduit_body::BodyState::Lulled
    );
    let resumed = super::super::resume_service(host("boot/clock-interval-next"), &root).unwrap();
    assert_eq!(resumed.session.evidence().body_id, body_id);
    assert_eq!(
        resumed.session.evidence().body.workset.plots(),
        &[replacement_resident]
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn terminal_show_returns_one_typed_clock_change_to_the_same_owner() {
    use conduit_presentation::{
        FaceInteraction, FaceInteractionArgument, PresentationActionAvailability,
    };
    use conduit_std_host::terminal_face_mask::{TerminalFaceMask, TerminalMaskExecution};
    use conduit_std_host::terminal_mask_execution::HostedTerminalMaskExecution;

    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "owner-clock-interaction-test",
        "terminal-show",
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
        selected_speech: None,
        selected_model: None,
    };
    super::super::super::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let checked = crate::plot_source::parse(CLOCK_SOURCE)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let mut owner =
        Owner::open(host("boot/interaction"), resident(&checked), None, "Clock").unwrap();
    owner.persist(&root).unwrap();
    std::fs::write(root.join("body/source.conduit"), CLOCK_SOURCE).unwrap();
    owner.set_resident_plot_name(&checked).unwrap();
    let face = owner.local_face_snapshot().unwrap();
    let action = face
        .actions
        .iter()
        .find(|action| action.intent == super::clock_interval_action())
        .unwrap();
    assert_eq!(
        action.availability,
        PresentationActionAvailability::Available
    );
    assert_eq!(action.arguments.len(), 1);
    let start = face
        .actions
        .iter()
        .find(|action| action.intent == super::clock_interval::CLOCK_START_ACTION)
        .unwrap();
    let stop = face
        .actions
        .iter()
        .find(|action| action.intent == super::clock_interval::CLOCK_LULL_ACTION)
        .unwrap();
    assert_eq!(
        start.availability,
        PresentationActionAvailability::Available
    );
    assert!(start.arguments.is_empty());
    assert!(matches!(
        stop.availability,
        PresentationActionAvailability::Unavailable { .. }
    ));
    let mut mask = TerminalFaceMask::prepare(face.clone(), 80, 24).unwrap();
    let mut execution = HostedTerminalMaskExecution::new(owner.host.advertisement()).unwrap();
    let mut output = Vec::new();
    mask.present(&mut execution, &mut output).unwrap();
    let show = mask.show().unwrap().clone();
    let start_interaction =
        FaceInteraction::new(&face, &show, &start.identity, &start.target, vec![], 0).unwrap();
    assert_eq!(
        owner
            .resolve_clock_interaction(&show, &start_interaction)
            .unwrap(),
        super::clock_interval::ClockAction::Start
    );
    let interaction = FaceInteraction::new(
        &face,
        &show,
        &action.identity,
        &action.target,
        vec![FaceInteractionArgument {
            name: action.arguments[0].name.clone(),
            value_kind: action.arguments[0].contract.value_kind.as_str().into(),
            value: b"500".to_vec(),
        }],
        1,
    )
    .unwrap();
    assert!(FaceInteraction::new(
        &face,
        &show,
        &action.identity,
        &action.target,
        vec![FaceInteractionArgument {
            name: action.arguments[0].name.clone(),
            value_kind: action.arguments[0].contract.value_kind.as_str().into(),
            value: b"3000".to_vec(),
        }],
        2,
    )
    .is_err());
    let correlated = execution.interact(interaction.clone()).unwrap();
    assert_eq!(correlated.interaction, interaction);
    let result = owner
        .apply_clock_interval_interaction(&root, &show, &correlated.interaction)
        .unwrap();
    assert_eq!(result["interval_ms"], 500);
    assert_eq!(result["next_step"], "start-replacement-plan");
    assert_eq!(result["prior_show_id"], show.show_id.as_str());
    assert!(owner
        .apply_clock_interval_interaction(&root, &show, &correlated.interaction)
        .is_err());
    assert!(owner
        .resolve_clock_interaction(&show, &start_interaction)
        .is_err());
    assert_eq!(owner.session.evidence().body.workload_revision, 2);
    assert_eq!(
        state::load(&root).unwrap().unwrap(),
        *owner.session.evidence()
    );
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
        selected_speech: None,
        selected_model: None,
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
        selected_speech: None,
        selected_model: None,
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
