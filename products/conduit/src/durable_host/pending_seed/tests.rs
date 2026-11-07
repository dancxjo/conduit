use super::*;
use crate::joining_host_package_test_common as common;
use crate::native_package_install::LocalNativeInstaller;
use conduit_host_make::{BodyBoundArtifactIdentity, NativePackageInstaller};

fn timer_configuration() -> conduit_host_make::CheckedHostConfiguration {
    let source = include_str!("../../../../../targets/std/profiles/linux-computer.host.conduit").replace("kind: \"clock/monotonic\"", "kind: \"timer/monotonic\"").replace("  limits =", "  capability = {kind: \"time/tick\", pinned_implementation: \"std/kernel-time-tick@2\"}\n  need = {id: \"timer/main\", class: \"conduit.resource/timer-slot@1\", slots: 8, bytes: 512}\n  limits =");
    conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(&source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap()
}

#[test]
fn configured_seed_initializes_exact_timer_before_admission() {
    let now = current_time_millis().unwrap();
    let configuration = timer_configuration();
    let (release, spore, invitation, payloads) = common::fixture_with_configuration_at(
        &["conduit-linux-x86_64", "install-linux-x86_64.sh"],
        false,
        now,
        &configuration,
    );
    let (comparison_manifest, comparison_state) = super::super::tests::fixture();
    let comparison_host = super::super::install(&comparison_manifest, &comparison_state).unwrap();
    let mut bytes = Vec::new();
    let receipt = conduit_body_make::write_joining_host_package(
        &release,
        &spore,
        &configuration,
        &invitation,
        now + 1,
        |path| Ok(std::io::Cursor::new(payloads[path].clone())),
        &mut bytes,
        || false,
    )
    .unwrap();
    let artifact = BodyBoundArtifactIdentity {
        target_id: spore.manifest.target.clone(),
        image_id: spore.manifest.image_id.clone(),
        image_content_sha256: receipt.image_content_sha256,
        spore_id: spore.manifest.spore_id.clone(),
        artifact_content_sha256: receipt.artifact_content_sha256,
        artifact_bytes: receipt.artifact_bytes,
    };
    let root = comparison_state.parent().unwrap().join("seed");
    let source = comparison_state.parent().unwrap().join("seed.zip");
    fs::write(&source, bytes).unwrap();
    let mut installer = LocalNativeInstaller::new_without_service_manager(root.clone());
    installer.install_and_start(&source, &artifact).unwrap();
    assert!(has_pending_seed(&root).unwrap());
    let selected = read_installation(&root.join("installation.json"))
        .unwrap()
        .selected_host_configuration
        .unwrap();
    assert_eq!(selected.host_image().unwrap(), &spore.image);
    assert_eq!(
        selected.configuration_id(),
        spore.manifest.host_configuration_id
    );
    assert_ne!(
        installer.installed_host().unwrap().host_id,
        comparison_host.host_id
    );
    assert!(installer.install_and_start(&source, &artifact).is_err());
    assert!(prepare_pending_seed_request(&root, false).is_err());
    let mut alternate = invitation.clone();
    alternate.secret = [91; 32];
    assert!(validate_invitation(&root, &alternate).is_err());
    assert!(validate_completion(&root, "body/foreign").is_err());
    let retained = read(&root).unwrap();
    assert_eq!(retained.invitation.claim, invitation.claim);
    assert_eq!(retained.invitation.secret, invitation.secret);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(root.join(PATH)).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    let (_, mut runtime) = super::super::runtime_preparation::prepare_runtime(&root).unwrap();
    let advertisement = runtime.truth().advertisement;
    assert_eq!(
        advertisement.profile.as_str(),
        spore.image.manifest.profile_id
    );
    assert_eq!(advertisement.capabilities.len(), 1);
    assert_eq!(
        advertisement.capabilities[0]
            .implementation
            .implementation_id
            .as_str(),
        conduit_std_offers::TICK_IMPLEMENTATION
    );
    assert_eq!(advertisement.resources.len(), 1);
    assert_eq!(advertisement.resources[0].capacity_units, 8);
    assert!(!advertisement.bases.iter().any(|base| matches!(
        base.mechanism_family.as_str(),
        "storage/protected-file" | "serial/text"
    )));
    assert!(runtime.prepare_birth_encounter(&root).is_err());
    assert!(root.join("runtime.json").exists());
    assert!(!root.join("control.sock").exists());
    assert!(!root.join("body/pending-join.json").exists());
    let installation = read_installation(&root.join("installation.json")).unwrap();
    assert!(installation.body_state.is_none());
    assert!(installation.joined_body_state.is_none());
    fs::remove_dir_all(comparison_state.parent().unwrap()).unwrap();
}

#[test]
fn expired_or_corrupt_seed_never_becomes_a_birth_or_implicit_admission() {
    let configuration = timer_configuration();
    let (release, spore, invitation, _) =
        common::fixture_with_configuration_at(&["conduit-linux-x86_64"], false, 0, &configuration);
    let (manifest, state) = super::super::tests::fixture();
    // This fixture exercises delayed Boot fencing; keep its synthetic release
    // source consistent with the actual selected Make IMAGE.
    let mut release_document: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    release_document["source_identity"] = spore.image.manifest.source_identity.clone().into();
    fs::write(&manifest, serde_json::to_vec(&release_document).unwrap()).unwrap();
    let source =
        conduit_host_make::canonical_host_configuration_conduit(configuration.configuration())
            .unwrap();
    install_pending_seed(
        &manifest,
        &state,
        &invitation,
        &source,
        configuration.configuration_id(),
        &spore.image,
        false,
    )
    .unwrap();
    assert!(has_pending_seed(&state).unwrap());
    assert!(prepare_pending_seed_request(&state, true).is_err());
    let (_, mut runtime) = super::super::runtime_preparation::prepare_runtime(&state).unwrap();
    assert!(runtime.prepare_birth_encounter(&state).is_err());
    assert!(read_installation(&state.join("installation.json"))
        .unwrap()
        .joined_body_state
        .is_none());
    let installation_path = state.join("installation.json");
    let original = fs::read(&installation_path).unwrap();
    let mut partial: serde_json::Value = serde_json::from_slice(&original).unwrap();
    partial
        .as_object_mut()
        .unwrap()
        .remove("selected_host_configuration");
    fs::write(&installation_path, serde_json::to_vec(&partial).unwrap()).unwrap();
    assert!(super::super::runtime_preparation::prepare_runtime(&state).is_err());
    fs::write(&installation_path, original).unwrap();
    fs::write(state.join(PATH), b"corrupt").unwrap();
    assert!(has_pending_seed(&state).is_err());
    assert!(super::super::runtime_preparation::prepare_runtime(&state).is_err());
    drop(release);
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[test]
fn legacy_checked_host_configuration_without_image_still_initializes() {
    let (manifest, state) = super::super::tests::fixture();
    super::super::install(&manifest, &state).unwrap();
    let source_path = state.parent().unwrap().join("legacy.host.conduit");
    fs::write(
        &source_path,
        include_str!("../../../../../targets/std/profiles/linux-computer.host.conduit"),
    )
    .unwrap();
    super::super::selected_host_configuration::configure(&state, Some(&source_path), false)
        .unwrap();
    let installation = read_installation(&state.join("installation.json")).unwrap();
    assert!(installation
        .selected_host_configuration
        .unwrap()
        .host_image()
        .is_none());
    let (_, runtime) = super::super::runtime_preparation::prepare_runtime(&state).unwrap();
    assert_eq!(
        runtime
            .truth()
            .advertisement
            .declared_name
            .as_ref()
            .map(|name| name.as_str()),
        Some("linux-computer")
    );
    assert!(state.join("runtime.json").exists());
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}
