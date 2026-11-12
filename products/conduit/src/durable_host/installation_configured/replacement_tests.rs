//! Explicit software refabrication never clears the installed identity.
use super::*;
const SOURCE: &str =
    include_str!("../../../../../targets/std/profiles/linux-computer.host.conduit");
fn image(source: &str, release: &str) -> conduit_host_make::HostImage {
    let checked =
        selected_host_configuration::Selection::from_source(source, running_target_id().unwrap())
            .unwrap()
            .checked()
            .unwrap();
    conduit_host_make::build_default_host_image(
        checked.profile().clone(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &conduit_host_make::BuildInputs {
            source_identity: release.into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0
}
#[test]
fn refabricated_install_verifies_all_artifacts_before_atomic_identity_preserving_commit() {
    let (manifest, state) = super::super::tests::fixture();
    let mut installed = install(&manifest, &state).unwrap();
    let old_image = state.join("old-image.json");
    write_json_atomic(&old_image, &image(SOURCE, "commit:fixture")).unwrap();
    installed.selected_host_configuration = selected_host_configuration::replacement_image(
        Some(
            selected_host_configuration::Selection::from_source(
                SOURCE,
                running_target_id().unwrap(),
            )
            .unwrap(),
        ),
        Some(&old_image),
        "commit:fixture",
    )
    .unwrap();
    let body = conduit_body::Body::born(
        "source/refabrication-fixture".into(),
        "checked/refabrication-fixture".into(),
        1,
        "sign/refabrication-fixture".into(),
    )
    .unwrap();
    let biography = conduit_body::BodyBiographyEvidence::born(
        body.clone(),
        conduit_body::BodyMembership::new(body.body_id.clone()).unwrap(),
        "Refabrication fixture".into(),
    )
    .unwrap();
    let biography_bytes = serde_json::to_vec(&biography).unwrap();
    let biography_path = state.join("biography.json");
    fs::write(&biography_path, &biography_bytes).unwrap();
    installed.body_state = Some(BodyBinding {
        body_id: body.body_id.as_str().into(),
        biography_sha256: digest(&biography_bytes),
        biography_path: biography_path.display().to_string(),
    });
    let installation = state.join("installation.json");
    write_json_atomic(&installation, &installed).unwrap();
    let original = fs::read(&installation).unwrap();
    let token = fs::read(state.join("control.token")).unwrap();
    let unit = fs::read(state.join("conduit-host.service")).unwrap();
    let source = SOURCE
        .replace("host linux-computer", "host linux-computer-refabricated")
        .replace("clock/monotonic", "timer/monotonic");
    let configuration = state.join("refabricated.host.conduit");
    fs::write(&configuration, &source).unwrap();
    let new_image = state.join("new-image.json");
    write_json_atomic(&new_image, &image(&source, "commit:refabricated")).unwrap();
    let mut release: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    release["source_identity"] = "commit:refabricated".into();
    fs::write(&manifest, serde_json::to_vec(&release).unwrap()).unwrap();
    let apply = |manifest: &Path, image: Option<&Path>| {
        install_configured_with_replacement(
            manifest,
            &state,
            selected_speech::Change::Preserve,
            selected_model::Change::Preserve,
            selected_todo::Change::Preserve,
            image,
            Some(&configuration),
        )
    };
    // Source-only configure still refuses changing a Seed-bound source.
    assert!(selected_host_configuration::configure(&state, Some(&configuration), false).is_err());
    assert!(apply(&manifest, None).is_err());
    assert!(apply(&state.join("missing-release.json"), Some(&new_image)).is_err());
    assert!(apply(&manifest, Some(&state.join("missing-image.json"))).is_err());
    assert!(apply(&manifest, Some(&old_image)).is_err());
    for target in [None, Some("std/aarch64/other-board")] {
        let mut wrong = release.clone();
        if let Some(target) = target {
            wrong["target_id"] = target.into();
        } else {
            wrong.as_object_mut().unwrap().remove("target_id");
        }
        fs::write(&manifest, serde_json::to_vec(&wrong).unwrap()).unwrap();
        assert!(apply(&manifest, Some(&new_image)).is_err());
    }
    let mut wrong = release.clone();
    wrong["source_identity"] = "commit:wrong-release".into();
    fs::write(&manifest, serde_json::to_vec(&wrong).unwrap()).unwrap();
    assert!(apply(&manifest, Some(&new_image)).is_err());
    assert_eq!(fs::read(&installation).unwrap(), original);
    assert_eq!(fs::read(state.join("control.token")).unwrap(), token);
    assert_eq!(fs::read(state.join("conduit-host.service")).unwrap(), unit);
    assert_eq!(fs::read(&biography_path).unwrap(), biography_bytes);
    fs::write(&manifest, serde_json::to_vec(&release).unwrap()).unwrap();
    let command = || crate::cli::HostServiceCommand::Install {
        manifest: manifest.clone(),
        host_image: Some(new_image.clone()),
        configuration: Some(configuration.clone()),
        state_dir: state.clone(),
        no_start: true,
        speech: Default::default(),
        model: Default::default(),
        todo_checkpoint: Default::default(),
    };
    let host_lock = owner_lock::acquire(&state).unwrap();
    assert!(dispatch(command()).is_err());
    drop(host_lock);
    let body_lock = owner_lock::body(&state).unwrap();
    assert!(dispatch(command()).is_err());
    drop(body_lock);
    assert_eq!(fs::read(&installation).unwrap(), original);
    let updated = apply(&manifest, Some(&new_image)).unwrap();
    assert_eq!(updated.host_id, installed.host_id);
    assert_eq!(
        serde_json::to_value(&updated.body_state).unwrap(),
        serde_json::to_value(&installed.body_state).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&updated.joined_body_state).unwrap(),
        serde_json::to_value(&installed.joined_body_state).unwrap()
    );
    assert_eq!(fs::read(state.join("control.token")).unwrap(), token);
    assert_eq!(fs::read(&biography_path).unwrap(), biography_bytes);
    assert_ne!(
        updated
            .selected_host_configuration
            .as_ref()
            .unwrap()
            .checked()
            .unwrap()
            .configuration_id(),
        installed
            .selected_host_configuration
            .as_ref()
            .unwrap()
            .checked()
            .unwrap()
            .configuration_id()
    );
    read_installation(&installation).unwrap();
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn active_pending_seed_refuses_refabrication_without_changing_original_provision() {
    let (manifest, state) = super::super::tests::fixture();
    let selected =
        selected_host_configuration::Selection::from_source(SOURCE, running_target_id().unwrap())
            .unwrap();
    let checked = selected.checked().unwrap();
    let (_, spore, invitation, _) =
        crate::joining_host_package_test_common::fixture_with_configuration_at(
            &["conduit-linux-x86_64"],
            false,
            0,
            &checked,
        );
    let canonical =
        conduit_host_make::canonical_host_configuration_conduit(checked.configuration()).unwrap();
    let mut release: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    release["source_identity"] = spore.image.manifest.source_identity.clone().into();
    fs::write(&manifest, serde_json::to_vec(&release).unwrap()).unwrap();
    pending_seed::install_pending_seed(
        &manifest,
        &state,
        &invitation,
        &canonical,
        checked.configuration_id(),
        &spore.image,
        false,
    )
    .unwrap();
    assert!(pending_seed::has_pending_seed(&state).unwrap());
    let paths = [
        "installation.json",
        "pending-seed.json",
        "control.token",
        "conduit-host.service",
    ];
    let originals: Vec<_> = paths
        .iter()
        .map(|path| fs::read(state.join(path)).unwrap())
        .collect();
    let release_count = fs::read_dir(state.join("releases")).unwrap().count();
    let source = canonical.replace("host linux-computer", "host linux-computer-refabricated");
    assert_ne!(source, canonical);
    let configuration = state.join("refabricated.host.conduit");
    let new_image = state.join("new-image.json");
    fs::write(&configuration, &source).unwrap();
    write_json_atomic(&new_image, &image(&source, "commit:refabricated")).unwrap();
    // A distinct verified bundle would require a new immutable release directory
    // if the early pending-Seed fence were missing.
    let payload = b"new-reviewed-executable-refabrication-fixture";
    fs::write(
        manifest.parent().unwrap().join("conduit-linux-x86_64"),
        payload,
    )
    .unwrap();
    let files = vec![ReleaseFile {
        path: "conduit-linux-x86_64".into(),
        bytes: payload.len() as u64,
        sha256: digest(payload),
    }];
    release["source_identity"] = "commit:refabricated".into();
    release["bundle_sha256"] = bundle_digest(&files).into();
    release["files"] = serde_json::to_value(&files).unwrap();
    fs::write(&manifest, serde_json::to_vec(&release).unwrap()).unwrap();
    let error = install_configured_with_replacement(
        &manifest,
        &state,
        selected_speech::Change::Preserve,
        selected_model::Change::Preserve,
        selected_todo::Change::Preserve,
        Some(&new_image),
        Some(&configuration),
    )
    .unwrap_err();
    assert!(error.contains("pending joining Seed"), "{error}");
    for (path, original) in paths.into_iter().zip(originals) {
        assert_eq!(fs::read(state.join(path)).unwrap(), original);
    }
    assert_eq!(
        fs::read_dir(state.join("releases")).unwrap().count(),
        release_count
    );
    assert!(pending_seed::has_pending_seed(&state).unwrap());
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}
