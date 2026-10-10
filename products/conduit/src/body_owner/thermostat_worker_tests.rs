use super::*;
use conduit_body::ResidentPlot;
use conduit_core::{HostId, OfferGeneration};
use conduit_std_host::StdHostConfig;
use conduit_thermostat_plot::{Command, Mode};

use crate::mask_test_common as common;

fn fixture(maximum_items: u16) -> (Owner, std::path::PathBuf) {
    let root = std::env::temp_dir().join(super::super::super::super::fresh_identity(
        "owner-thermostat-test",
        "scan",
    ));
    std::fs::create_dir_all(&root).unwrap();
    let installation = super::super::super::super::Installation {
        schema: super::super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/thermostat-owner".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: super::super::super::super::digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_todo_checkpoint: None,
    };
    super::super::super::super::write_json_atomic(&root.join("installation.json"), &installation)
        .unwrap();
    let text = include_str!("../../../../plots/thermostat/main.conduit").replace(
        "maximum-items = 256",
        &format!("maximum-items = {maximum_items}"),
    );
    let checked = crate::plot_source::parse(&text)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let host = StdHost::new_for_thermostat_scan(
        StdHostConfig {
            host_id: HostId::from("host/thermostat-owner"),
            boot_id: "boot/thermostat-owner".into(),
            offer_generation: OfferGeneration(1),
        },
        &ThermostatState::default(),
        maximum_items,
    )
    .unwrap();
    let mut owner = Owner::open(
        host,
        ResidentPlot::new(
            checked.expanded.source_document_id.clone(),
            checked.expanded.checked_plot_id.clone(),
        ),
        None,
        "Living room",
    )
    .unwrap();
    owner.set_resident_plot_name(&checked).unwrap();
    owner.persist(&root).unwrap();
    std::fs::write(root.join("body/source.conduit"), &text).unwrap();
    (owner, root)
}

#[test]
fn installed_thermostat_owner_projects_and_routes_one_retained_scan() {
    let (mut owner, root) = fixture(256);
    let mut worker = owner.start_thermostat_run(&root, 5_000).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while owner.current_play_id().is_none() {
        assert!(Instant::now() < deadline);
        assert!(!worker.progress(&mut owner, &root).unwrap());
        thread::sleep(Duration::from_millis(5));
    }
    assert!(owner.host.is_playing());
    let play = owner.current_play_id().unwrap().clone();
    let face = owner.local_face_snapshot().unwrap();
    assert_eq!(
        face.basis.body_id.as_ref(),
        Some(&owner.session.evidence().body_id)
    );
    let show = common::available_mask_show(&face);
    let action = face
        .actions
        .iter()
        .find(|action| action.identity == "thermostat.mode.heat")
        .unwrap();
    // This mechanical spoken encounter consumes the exact Owner Face and
    // acknowledged Show; it contributes no thermostat state or Host.
    let mut spoken =
        conduit_std_host::spoken_face_mask::SpokenFaceSession::new(face.clone(), show.clone())
            .unwrap();
    spoken
        .command(
            &face,
            &show,
            conduit_std_host::spoken_face_mask::ReaderCommand::FocusAction(action.identity.clone()),
            1,
        )
        .unwrap();
    let interaction = spoken
        .command(
            &face,
            &show,
            conduit_std_host::spoken_face_mask::ReaderCommand::Activate,
            2,
        )
        .unwrap()
        .interaction
        .expect("spoken activation returns a typed interaction");
    assert_eq!(
        owner
            .resolve_thermostat_interaction(&show, &interaction)
            .unwrap(),
        Command::SetMode(Mode::Heat)
    );
    assert!(matches!(
        worker
            .submit_interaction(&owner, &show, &interaction)
            .unwrap(),
        BodyLiveForeAdmission::Accepted { .. }
    ));
    assert!(worker
        .submit_interaction(&owner, &show, &interaction)
        .is_err());
    while worker.output_pending() {
        assert!(Instant::now() < deadline);
        assert!(!worker.progress(&mut owner, &root).unwrap());
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(owner.current_play_id(), Some(&play));
    assert_eq!(
        owner.thermostat_live.as_ref().unwrap().state.mode,
        Mode::Heat
    );
    let next_face = owner.local_face_snapshot().unwrap();
    assert!(next_face.revision > face.revision);
    assert!(owner
        .resolve_thermostat_interaction(&show, &interaction)
        .is_err());
    let next_show = common::available_mask_show(&next_face);
    let action = next_face
        .actions
        .iter()
        .find(|action| action.identity == "thermostat.mode.heat")
        .unwrap();
    let noop = FaceInteraction::new(
        &next_face,
        &next_show,
        &action.identity,
        &action.target,
        vec![],
        2,
    )
    .unwrap();
    worker
        .submit_interaction(&owner, &next_show, &noop)
        .unwrap();
    while worker.output_pending() {
        assert!(Instant::now() < deadline);
        assert!(!worker.progress(&mut owner, &root).unwrap());
        thread::sleep(Duration::from_millis(5));
    }
    assert!(owner.local_face_snapshot().unwrap().revision > next_face.revision);
    worker.request_lull().unwrap();
    while !worker.progress(&mut owner, &root).unwrap() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    assert!(!owner.host.is_playing());
    assert!(owner.current_play_id().is_none());
    assert!(owner.thermostat_live.is_none());
    assert_eq!(
        owner.last_execution.as_ref().unwrap()["terminal"]["Cancelled"]["reason"],
        "OperatorRequested"
    );
    assert!(owner.last_execution.as_ref().unwrap()["failure"].is_null());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn installed_thermostat_terminal_drain_observes_final_fore() {
    let (mut owner, root) = fixture(1);
    let mut worker = owner.start_thermostat_run(&root, 5_000).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while owner.current_play_id().is_none() {
        assert!(Instant::now() < deadline);
        assert!(!worker.progress(&mut owner, &root).unwrap());
        thread::sleep(Duration::from_millis(5));
    }
    // Model an empty first output poll, then a final value arriving before
    // terminal observation. Retiring must drain that value independently.
    worker.observe_output(&mut owner).unwrap();
    let face = owner.local_face_snapshot().unwrap();
    let show = common::available_mask_show(&face);
    let action = face
        .actions
        .iter()
        .find(|a| a.identity == "thermostat.mode.heat")
        .unwrap();
    let interaction =
        FaceInteraction::new(&face, &show, &action.identity, &action.target, vec![], 1).unwrap();
    worker
        .submit_interaction(&owner, &show, &interaction)
        .unwrap();
    while !worker.thread.as_ref().unwrap().is_finished() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    assert!(worker.output_pending());
    assert!(worker.finish_if_ready(&mut owner, &root).unwrap());
    assert!(!worker.output_pending());
    let receipt = owner.last_execution.as_ref().unwrap();
    assert_eq!(receipt["terminal"], "Completed");
    assert_eq!(receipt["observed_outputs"], 1);
    assert_eq!(receipt["pending_output"], false);
    assert_eq!(receipt["state"]["mode"], "Heat");
    assert!(receipt["failure"].is_null());
    assert!(!owner.host.is_playing());
    assert!(owner.thermostat_live.is_none());
    std::fs::remove_dir_all(root).unwrap();
}
