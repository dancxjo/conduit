//! Actual attached provider → bounded Mask Fore → owner workset replacement.
use super::*;
use conduit_face::{FaceInteraction, FaceInteractionArgument};

#[test]
fn attached_action_replaces_lulled_workset_and_cannot_replay_after_new_show() {
    let mut runtime = runtime();
    let state = state(&runtime);
    let token = [7; 32];
    persist_fixture(
        &mut runtime,
        &state,
        include_str!("../../../../../plots/clock/main.conduit"),
    );

    let (client, plan, show) = attach_once(&mut runtime, &state, &token);
    let HostSource::Body { owner, .. } = &runtime.host else {
        unreachable!()
    };
    let face = owner.local_face_snapshot().unwrap();
    let body_id = face.basis.body_id.clone();
    assert!(face.basis.wake_id.is_none());
    assert!(face.basis.plan_id.is_none());
    assert!(face.basis.active_play_id.is_none());
    let action = face
        .actions
        .iter()
        .find(|a| a.intent == crate::durable_host::owner::clock_interval_action())
        .unwrap();
    assert!(action.availability.is_available());
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
    let result = runtime
        .attached_terminal_interaction(&plan, &show, interaction.clone())
        .unwrap();
    assert_eq!(result["interval_ms"], 500);
    assert_eq!(result["play_state"], "lulled");
    assert!(runtime.terminal_route.is_none());
    assert!(runtime
        .attached_terminal_interaction(&plan, &show, interaction.clone())
        .is_err());
    drop(client);
    retire_closed_attachment(&state, &mut runtime).unwrap();

    let (client, replacement, next_show) = attach_once(&mut runtime, &state, &token);
    assert_ne!(replacement, plan);
    assert_ne!(next_show.show_id, show.show_id);
    let HostSource::Body { owner, .. } = &runtime.host else {
        unreachable!()
    };
    let next_face = owner.local_face_snapshot().unwrap();
    assert_eq!(next_face.basis.body_id, body_id);
    assert!(next_face.basis.wake_id.is_none());
    assert!(next_face.basis.plan_id.is_none());
    assert!(next_face.basis.active_play_id.is_none());
    assert_ne!(next_face.identity, face.identity);
    assert!(interaction
        .validate_against(&next_face, &next_show)
        .is_err());
    assert!(runtime
        .attached_terminal_interaction(&replacement, &next_show, interaction)
        .is_err());
    assert!(runtime.terminal_route.is_none());
    let retained: serde_json::Value =
        serde_json::from_slice(&fs::read(state.join("body/workset-sources.json")).unwrap())
            .unwrap();
    assert_eq!(retained["schema"], "conduit.body/workset-sources@1");
    let entries = retained["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["entry"], "clock-demo");
    assert!(entries[0]["source"].as_str().unwrap().contains("500ms"));
    drop(client);
    retire_closed_attachment(&state, &mut runtime).unwrap();
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn actual_terminal_seal_refuses_matching_ids_without_back_or_resource() {
    use conduit_face::LocalOwnerMaskRouteError;
    let mut runtime = runtime();
    let state = state(&runtime);
    let token = [7; 32];
    let (client, _, _) = attach_once(&mut runtime, &state, &token);
    let seal = &runtime.terminal_route.as_ref().unwrap().seal;
    let mut missing_back = seal.clone();
    missing_back.owner_offer.capabilities.clear();
    assert_eq!(
        missing_back.verify_seal(),
        Err(LocalOwnerMaskRouteError::StaleOrMissingOffer)
    );
    let mut missing_resource = seal.clone();
    missing_resource.owner_offer.resources.clear();
    assert_eq!(
        missing_resource.verify_seal(),
        Err(LocalOwnerMaskRouteError::StaleOrMissingOffer)
    );
    let mut changed_boot = seal.clone();
    changed_boot.owner_offer.boot_id = "boot/replacement".into();
    assert_eq!(
        changed_boot.verify_seal(),
        Err(LocalOwnerMaskRouteError::RemoteLineRequired)
    );
    drop(client);
    retire_closed_attachment(&state, &mut runtime).unwrap();
    fs::remove_dir_all(state).unwrap();
}

fn persist_fixture(runtime: &mut DurableHostRuntime, state: &Path, source: &str) {
    let HostSource::Body { owner, root, .. } = &mut runtime.host else {
        unreachable!()
    };
    *root = state.to_path_buf();
    fs::write(
        state.join("installation.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema": "conduit.install/durable-host@1",
            "host_id": owner.host.advertisement().host_id.as_str(),
            "release_source_identity": "source/test",
            "release_bundle_sha256": format!("sha256:{}", "0".repeat(64)),
            "product_executable": "fixture-unused",
            "body_state": null,
            "joined_body_state": null,
            "selected_speech": null
        }))
        .unwrap(),
    )
    .unwrap();
    owner.persist(state).unwrap();
    fs::write(state.join("body/source.conduit"), source).unwrap();
}

#[test]
fn attached_wake_retires_mask_before_start_and_cannot_replay() {
    let source = include_str!("../../../../../plots/clock/main.conduit")
        .replace("plot clock-demo", "plot terminal-wake");
    let mut runtime = runtime_with_source(&source);
    let state = state(&runtime);
    persist_fixture(&mut runtime, &state, &source);
    let (client, plan, show) = attach_once(&mut runtime, &state, &[7; 32]);
    let HostSource::Body { owner, .. } = &runtime.host else {
        unreachable!()
    };
    let face = owner.local_face_snapshot().unwrap();
    let wake = face
        .actions
        .iter()
        .find(|a| a.intent == "conduit.intent/wake@1")
        .unwrap();
    assert!(wake.availability.is_available());
    let interaction =
        FaceInteraction::new(&face, &show, &wake.identity, &wake.target, vec![], 1).unwrap();
    let result = runtime
        .attached_terminal_interaction(&plan, &show, interaction.clone())
        .unwrap();
    assert_eq!(result["play_state"], "preparing");
    assert!(runtime.terminal_route.is_none());
    assert!(!is_attached(&mut runtime));
    assert!(matches!(
        &runtime.host,
        HostSource::Body {
            running: Some(_),
            ..
        }
    ));
    assert!(runtime
        .attached_terminal_interaction(&plan, &show, interaction)
        .is_err());
    drop(client);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        runtime.progress_owned_body().unwrap();
        if matches!(&runtime.host, HostSource::Body { owner, .. } if owner.current_play_id().is_some())
        {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    runtime.request_owned_body_lull().unwrap();
    loop {
        runtime.progress_owned_body().unwrap();
        if matches!(&runtime.host, HostSource::Body { running: None, .. }) {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    fs::remove_dir_all(state).unwrap();
}
