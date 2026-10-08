//! Explicit recovery of persisted selection, without executable legacy coverage.
use super::*;
use std::path::PathBuf;

fn legacy() -> (PathBuf, PathBuf, String) {
    let (manifest, state) = super::tests::fixture();
    let installed = install_configured(
        &manifest,
        &state,
        selected_speech::Change::Replace(selected_speech::fixture_retained_selection()),
        selected_model::Change::Preserve,
    )
    .unwrap();
    let path = state.join("installation.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let body = conduit_body::Body::born(
        "source/selection-recovery".into(),
        "checked/selection-recovery".into(),
        1,
        conduit_core::SignId::from("sign/selection-recovery/born"),
    )
    .unwrap();
    let biography = conduit_body::BodyBiographyEvidence::born(
        body.clone(),
        conduit_body::BodyMembership::new(body.body_id.clone()).unwrap(),
        "Selection recovery".into(),
    )
    .unwrap();
    let body_dir = state.join("body");
    fs::create_dir_all(&body_dir).unwrap();
    let biography_path = body_dir.join("biography.json");
    let bytes = serde_json::to_vec_pretty(&biography).unwrap();
    fs::write(&biography_path, &bytes).unwrap();
    value["body_state"] = serde_json::json!({
        "body_id": body.body_id.as_str(),
        "biography_path": biography_path.display().to_string(),
        "biography_sha256": digest(&bytes)
    });
    value["selected_speech"]
        .as_object_mut()
        .unwrap()
        .remove("language_coverage");
    write_json_atomic(&path, &value).unwrap();
    (manifest, state, installed.host_id)
}

#[test]
fn legacy_selection_requires_explicit_replacement_or_removal() {
    for change in [
        selected_speech::Change::Remove,
        selected_speech::Change::Replace(selected_speech::fixture_retained_selection()),
    ] {
        let (manifest, state, host_id) = legacy();
        assert!(read_installation(&state.join("installation.json"))
            .unwrap_err()
            .contains("reselect"));
        assert!(install(&manifest, &state).unwrap_err().contains("reselect"));
        assert!(start_runtime(&state).unwrap_err().contains("reselect"));
        let before: serde_json::Value =
            serde_json::from_slice(&fs::read(state.join("installation.json")).unwrap()).unwrap();
        let repaired =
            install_configured(&manifest, &state, change, selected_model::Change::Preserve)
                .expect("explicit selection recovery");
        assert_eq!(repaired.host_id, host_id);
        assert_eq!(
            serde_json::to_value(&repaired).unwrap()["body_state"],
            before["body_state"]
        );
        assert_eq!(
            read_installation(&state.join("installation.json"))
                .unwrap()
                .host_id,
            host_id
        );
        fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }
}

#[test]
fn selection_recovery_cannot_bypass_body_identity_validation() {
    let (manifest, state, _) = legacy();
    let path = state.join("installation.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["body_state"] = serde_json::json!({
        "body_id": "body/stale", "biography_path": "/missing/biography.json", "biography_sha256": "invalid"
    });
    write_json_atomic(&path, &value).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(install_configured(
        &manifest,
        &state,
        selected_speech::Change::Remove,
        selected_model::Change::Preserve,
    )
    .is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn replacement_cannot_reuse_undeclared_legacy_selection() {
    let (manifest, state, _) = legacy();
    let path = state.join("installation.json");
    let before = fs::read(&path).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&before).unwrap();
    let unreviewed = serde_json::from_value(value["selected_speech"].clone()).unwrap();
    assert!(install_configured(
        &manifest,
        &state,
        selected_speech::Change::Replace(unreviewed),
        selected_model::Change::Preserve,
    )
    .unwrap_err()
    .contains("reselect"));
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn todo_selection_survives_reinstall_and_refuses_rebound_root_before_boot() {
    let (manifest, state) = super::tests::fixture();
    let root = state.join("todo-checkpoint");
    fs::create_dir_all(&root).unwrap();
    let selected = selected_todo::Selection::select(&root, None).unwrap();
    let version = selected.version_hex();
    let first = install_configured_with_todo(
        &manifest,
        &state,
        selected_speech::Change::Preserve,
        selected_model::Change::Preserve,
        selected_todo::Change::Replace(selected),
    )
    .unwrap();
    let before = fs::read(state.join("installation.json")).unwrap();
    assert!(install_configured_with_todo(
        &manifest,
        &state,
        selected_speech::Change::Replace(selected_speech::fixture_retained_selection()),
        selected_model::Change::Preserve,
        selected_todo::Change::Preserve,
    )
    .unwrap_err()
    .contains("cannot compose"));
    assert_eq!(fs::read(state.join("installation.json")).unwrap(), before);
    assert!(inspect_installation(&state.join("installation.json"))
        .unwrap()
        .contains(&version));
    let retained = install(&manifest, &state).unwrap();
    assert_eq!(first.host_id, retained.host_id);
    let selected = selected_todo_checkpoint(&state).unwrap().unwrap();
    assert_eq!(selected.root, root.canonicalize().unwrap());
    assert_eq!(
        selected.content.version.digest(),
        decode_todo_version(&version)
    );
    let (_status, truth) = prepare_runtime(&state).unwrap();
    assert!(truth
        .into_owner_host()
        .advertisement()
        .resources
        .iter()
        .any(|resource| resource.pool_id.as_str() == "std/todo-checkpoint"));
    fs::remove_file(state.join("runtime.json")).unwrap();
    let moved = root.with_extension("moved");
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    #[cfg(unix)]
    assert!(start_runtime(&state)
        .unwrap_err()
        .contains("root identity changed"));
    assert!(!state.join("runtime.json").exists());
    install_configured_with_todo(
        &manifest,
        &state,
        selected_speech::Change::Preserve,
        selected_model::Change::Preserve,
        selected_todo::Change::Remove,
    )
    .unwrap();
    assert!(selected_todo_checkpoint(&state).unwrap().is_none());
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

fn decode_todo_version(hex: &str) -> [u8; 32] {
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
    }
    bytes
}
