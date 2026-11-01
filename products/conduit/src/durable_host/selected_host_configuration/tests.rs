use super::super::{digest, fs, install, write_json_atomic};
use super::*;
const SOURCE: &str =
    include_str!("../../../../../targets/std/profiles/linux-computer.host.conduit");
#[test]
fn retained_source_requires_exact_checked_identity_and_actual_target() {
    let selection = Selection::from_source(SOURCE, "std/x86_64/computer").unwrap();
    selection.validate().unwrap();
    assert!(Selection::from_source(SOURCE, "std/aarch64/other-board").is_err());
    let mut changed = selection.clone();
    changed.configuration_id = "different".into();
    assert!(changed.validate().is_err());
    changed.source = "x".repeat(MAXIMUM_SOURCE_BYTES as usize + 1);
    assert!(changed.validate().is_err());
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn explicit_base_only_purpose_is_distinct_from_legacy_software_composition() {
    let legacy = Selection::from_source(SOURCE, "std/x86_64/computer").unwrap();
    let fallback = StdHostComposition::minimal().with_time();
    assert_eq!(legacy.composition(fallback).unwrap(), fallback);
    let mut configuration = parse_host_configuration_conduit(SOURCE).unwrap();
    configuration.software = Some(conduit_host_make::HostSoftwarePurpose {
        capabilities: Vec::new(),
    });
    let source = canonical_host_configuration_conduit(&configuration).unwrap();
    let selected = Selection::from_source(&source, "std/x86_64/computer").unwrap();
    assert_ne!(legacy.configuration_id, selected.configuration_id);
    assert_eq!(
        selected.composition(fallback).unwrap(),
        StdHostComposition::minimal()
    );
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn configure_reinstall_and_clear_preserve_durable_host_identity() {
    let (manifest, state) = super::super::tests::fixture();
    let mut initial = install(&manifest, &state).unwrap();
    let body = conduit_body::Body::born(
        "source/configuration-fixture".into(),
        "checked/configuration-fixture".into(),
        1,
        "sign/configuration-fixture".into(),
    )
    .unwrap();
    let biography = conduit_body::BodyBiographyEvidence::born(
        body.clone(),
        conduit_body::BodyMembership::new(body.body_id.clone()).unwrap(),
        "Configuration fixture".into(),
    )
    .unwrap();
    let biography_bytes = serde_json::to_vec(&biography).unwrap();
    let biography_path = state.join("biography.json");
    fs::write(&biography_path, &biography_bytes).unwrap();
    initial.body_state = Some(super::super::BodyBinding {
        body_id: body.body_id.as_str().into(),
        biography_sha256: digest(&biography_bytes),
        biography_path: biography_path.display().to_string(),
    });
    write_json_atomic(&state.join("installation.json"), &initial).unwrap();
    let source = state.join("selection.host.conduit");
    fs::write(&source, SOURCE).unwrap();
    configure(&state, Some(&source), false).unwrap();
    let selected = read_installation(&state.join("installation.json")).unwrap();
    assert_eq!(selected.host_id, initial.host_id);
    assert!(selected.selected_host_configuration.is_some());
    let retained = install(&manifest, &state).unwrap();
    assert_eq!(
        selected.selected_host_configuration,
        retained.selected_host_configuration
    );
    assert_eq!(selected.host_id, retained.host_id);
    assert_eq!(
        serde_json::to_value(&selected.body_state).unwrap(),
        serde_json::to_value(&retained.body_state).unwrap()
    );
    configure(&state, None, true).unwrap();
    let cleared = read_installation(&state.join("installation.json")).unwrap();
    assert!(cleared.selected_host_configuration.is_none());
    assert_eq!(cleared.host_id, initial.host_id);
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn active_service_lock_and_invalid_selection_leave_installation_unchanged() {
    let (manifest, state) = super::super::tests::fixture();
    install(&manifest, &state).unwrap();
    let source = state.join("selection.host.conduit");
    fs::write(&source, SOURCE).unwrap();
    let path = state.join("installation.json");
    let before = fs::read(&path).unwrap();
    let lock = owner_lock::acquire(&state).unwrap();
    assert!(configure(&state, Some(&source), false).is_err());
    drop(lock);
    fs::write(&source, "invalid").unwrap();
    assert!(configure(&state, Some(&source), false).is_err());
    assert_eq!(before, fs::read(&path).unwrap());
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn explicit_clear_repairs_retained_configuration_without_relaxing_normal_reads() {
    let (manifest, state) = super::super::tests::fixture();
    let mut installation = install(&manifest, &state).unwrap();
    let mut selection = Selection::from_source(SOURCE, "std/x86_64/computer").unwrap();
    selection.configuration_id = "changed".into();
    installation.selected_host_configuration = Some(selection);
    write_json_atomic(&state.join("installation.json"), &installation).unwrap();
    assert!(read_installation(&state.join("installation.json")).is_err());
    configure(&state, None, true).unwrap();
    assert_eq!(
        read_installation(&state.join("installation.json"))
            .unwrap()
            .host_id,
        installation.host_id
    );
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod microphone_fixture {
    use super::*;
    use conduit_host_make::{BaseInitialization, ConfigurationBase};
    use conduit_std_host::{hosted_microphone::AlsaMicrophoneDiscovery, StdHostConfig};
    use std::os::unix::fs::PermissionsExt;
    fn selection(executable: &Path, base_identity: &str) -> Selection {
        let mut configuration = parse_host_configuration_conduit(SOURCE).unwrap();
        configuration.software = Some(conduit_host_make::HostSoftwarePurpose {
            capabilities: Vec::new(),
        });
        let initialization: BaseInitialization = serde_json::from_value(serde_json::json!({
            "executable": executable, "executable_sha256": digest(&fs::read(executable).unwrap()).trim_start_matches("sha256:"),
            "card_id":"ConduitFixture", "device":0, "base_identity":base_identity,
            "capture_milliseconds":1, "timeout_milliseconds":1000
        }))
        .unwrap();
        configuration.bases.push(ConfigurationBase {
            kind: "audio/microphone".into(),
            implementation: Some(conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION.into()),
            implementations: Vec::new(),
            initialization: Some(initialization),
        });
        let source = canonical_host_configuration_conduit(&configuration).unwrap();
        Selection::from_source(&source, "std/x86_64/computer").unwrap()
    }
    #[test]
    fn checked_selection_registers_actual_adapter_without_capture_and_refuses_drift() {
        let (manifest, state) = super::super::super::tests::fixture();
        install(&manifest, &state).unwrap();
        let executable = state.join("fixture-arecord");
        fs::write(&executable, "#!/bin/sh\nif [ \"$1\" = \"--capture\" ]; then shift; fi\nif [ \"$1\" = \"-l\" ]; then\necho observed >> \"$0.observed\"\nprintf 'card 999: ConduitFixture [Fixture], device 0: Capture [Capture]\\n'\nelse\necho capture-started > \"$0.capture\"\necho capture-must-not-start >&2\nexit 97\nfi\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let discovery = AlsaMicrophoneDiscovery::inspect(&executable).unwrap();
        fs::remove_file(state.join("fixture-arecord.observed")).unwrap();
        let selected = selection(&executable, &discovery.observations[0].base_identity);
        selected.validate().unwrap();
        let configuration_path = state.join("microphone.host.conduit");
        fs::write(&configuration_path, &selected.source).unwrap();
        configure(&state, Some(&configuration_path), false).unwrap();
        assert!(!state.join("fixture-arecord.observed").exists());
        assert!(!state.join("fixture-arecord.capture").exists());

        // A catalog-valid software selection is not permission to approximate
        // unsupported construction or probe the configured microphone.
        let before = fs::read(state.join("installation.json")).unwrap();
        let mut unsupported = parse_host_configuration_conduit(&selected.source).unwrap();
        let offer = conduit_std_offers::text_literal_offer();
        unsupported.software = Some(conduit_host_make::HostSoftwarePurpose {
            capabilities: vec![conduit_host_make::CapabilitySelection {
                kind: offer.kind_id.as_str().into(),
                contract_revision: offer.kind_contract_revision.as_str().into(),
                implementation: offer.implementation.implementation_id.as_str().into(),
            }],
        });
        check_host_configuration(
            unsupported.clone(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
        )
        .unwrap();
        let unsupported_source = state.join("unsupported-software.host.conduit");
        fs::write(
            &unsupported_source,
            canonical_host_configuration_conduit(&unsupported).unwrap(),
        )
        .unwrap();
        assert!(configure(&state, Some(&unsupported_source), false)
            .unwrap_err()
            .contains("cannot construct the selected software purpose"));
        assert_eq!(before, fs::read(state.join("installation.json")).unwrap());
        assert!(!state.join("fixture-arecord.observed").exists());
        assert!(!state.join("fixture-arecord.capture").exists());

        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: "host/installed-microphone-fixture".into(),
            boot_id: "boot/installed-microphone-fixture".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        });
        assert!(!host.advertisement().capabilities.iter().any(|v| v
            .implementation
            .implementation_id
            .as_str()
            == conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION));
        selected.attach_to_fresh_host(&mut host).unwrap();
        let installation = read_installation(&state.join("installation.json")).unwrap();
        let (status, runtime) =
            super::super::super::runtime_preparation::prepare_runtime(&state).unwrap();
        assert_eq!(status.host_id, installation.host_id);
        // The ordinary installed constructor builds the selected baseline before
        // attaching the actual discovered provider, without trimming an offer.
        assert_eq!(runtime.truth().advertisement.capabilities.len(), 3);
        assert!(
            serde_json::to_vec(&runtime.truth().advertisement)
                .unwrap()
                .len()
                <= 64 * 1024
        );
        assert_eq!(
            runtime.truth().advertisement.offer_generation.0,
            status.offer_generation
        );
        assert!(runtime.truth().advertisement.capabilities.iter().any(|v| v
            .implementation
            .implementation_id
            .as_str()
            == conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION));

        // The checked name and initialized microphone each advance the offer.
        assert_eq!(host.advertisement().offer_generation.0, 3);
        assert_eq!(
            host.advertisement().display_name(),
            parse_host_configuration_conduit(&selected.source)
                .unwrap()
                .name
        );
        assert!(host.advertisement().capabilities.iter().any(|v| v
            .implementation
            .implementation_id
            .as_str()
            == conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION));
        assert!(!state.join("fixture-arecord.capture").exists());
        let duplicate_base = selected
            .source
            .lines()
            .find(|line| line.contains("audio/microphone"))
            .unwrap();
        let duplicate_source = selected
            .source
            .replace("  limits =", &format!("{duplicate_base}\n  limits ="));
        let duplicate = Selection::from_source(&duplicate_source, "std/x86_64/computer").unwrap();
        let mut duplicate_host = StdHost::default();
        duplicate.attach_to_fresh_host(&mut duplicate_host).unwrap();
        assert_eq!(duplicate_host.advertisement().offer_generation.0, 3);
        let converted = Selection::from_source(
            &selected.source.replace(
                conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION,
                conduit_std_offers::CONVERTED_MICROPHONE_CLIP_IMPLEMENTATION,
            ),
            "std/x86_64/computer",
        )
        .unwrap();
        assert_ne!(selected.configuration_id, converted.configuration_id);
        let mut converted_host = StdHost::default();
        converted.attach_to_fresh_host(&mut converted_host).unwrap();
        assert!(converted_host.advertisement().capabilities.iter().any(|v| v
            .implementation
            .implementation_id
            .as_str()
            == conduit_std_offers::CONVERTED_MICROPHONE_CLIP_IMPLEMENTATION));
        assert!(!state.join("fixture-arecord.capture").exists());
        let changed_endpoint = selection(&executable, "wrong-endpoint");
        assert!(changed_endpoint
            .attach_to_fresh_host(&mut StdHost::default())
            .is_err());
        fs::write(
            &executable,
            "#!/bin/sh\necho unexpected > \"$0.drifted\"\nexit 98\n",
        )
        .unwrap();
        assert!(selected
            .attach_to_fresh_host(&mut StdHost::default())
            .unwrap_err()
            .contains("executable identity changed"));
        assert!(!state.join("fixture-arecord.drifted").exists());
        fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }
}

#[test]
fn checked_speaker_refuses_retained_physical_speech_overlap_without_provider_effects() {
    let source=SOURCE.replace("  limits =",r#"  base = {kind: "audio/speaker", implementation: "std/kernel-audio-play-alsa-hw@1", initialization: {executable_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", card_id: "Headphones", device: 0, base_identity: "bcm2835-audio"}}
  limits ="#);
    let selection = Selection::from_source(&source, "std/x86_64/computer").unwrap();
    selection.validate().unwrap();
    selection.validate_speech_overlap(None).unwrap();
    let speech = super::super::selected_speech::fixture_retained_selection();
    assert!(selection.validate_speech_overlap(Some(&speech)).is_err());
    let mut artifact = serde_json::to_value(speech).unwrap();
    artifact["artifact_only"] = serde_json::json!(true);
    let artifact = serde_json::from_value(artifact).unwrap();
    selection.validate_speech_overlap(Some(&artifact)).unwrap();
    let no_speaker = Selection::from_source(SOURCE, "std/x86_64/computer").unwrap();
    no_speaker.validate_speech_overlap(Some(&artifact)).unwrap();
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn checked_declaration_name_reaches_fresh_installed_offer_without_identity_change() {
    let source = SOURCE.replace("host linux-computer", "host Yellow");
    let selection = Selection::from_source(&source, "std/x86_64/computer").unwrap();
    let mut host = conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: "host/retained".into(),
        boot_id: "boot/current".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let before = host.advertisement().clone();
    selection.attach_to_fresh_host(&mut host).unwrap();
    let after = host.advertisement();
    assert_eq!(after.display_name(), "Yellow");
    assert_eq!(after.host_id, before.host_id);
    assert_eq!(after.boot_id, before.boot_id);
    assert_eq!(after.capabilities, before.capabilities);
    assert_eq!(after.resources, before.resources);
    assert_eq!(after.offer_generation.0, before.offer_generation.0 + 1);
}

#[cfg(all(
    feature = "installed-body-storage",
    target_os = "linux",
    target_arch = "x86_64"
))]
fn storage_source(root: &std::path::Path) -> String {
    SOURCE.replace(r#"base = {kind: "storage/protected-file", implementation: "hosted/protected-file@1"}"#,
        &format!(r#"base = {{kind: "storage/protected-file", implementation: "linux/body-storage-directory@1", initialization: {{root_directory: {:?}, pool_namespace: "body/storage", maximum_snapshot_records: 2, maximum_record_bytes: 4096, generation_slots: 2, publication_slots: 1, reader_leases: 1, state_slots: 1, maximum_state_bytes: 1024, total_disk_bytes: 65536, preparation_bytes: 8192}}}}"#,root.to_str().unwrap()))
}
#[cfg(all(
    feature = "installed-body-storage",
    target_os = "linux",
    target_arch = "x86_64"
))]
#[test]
fn storage_check_is_pure_and_failed_explicit_selection_never_persists_half_binding() {
    let (manifest, state) = super::super::tests::fixture();
    install(&manifest, &state).unwrap();
    let missing = state.join("not-created");
    let source = storage_source(&missing);
    let checked = check(&source).unwrap();
    let canonical = canonical_host_configuration_conduit(checked.configuration()).unwrap();
    assert_eq!(
        check(&canonical).unwrap().configuration_id(),
        checked.configuration_id()
    );
    assert!(!missing.exists());
    let source_path = state.join("storage.host.conduit");
    fs::write(&source_path, &source).unwrap();
    let before = fs::read(state.join("installation.json")).unwrap();
    assert!(configure(&state, Some(&source_path), false).is_err());
    assert_eq!(fs::read(state.join("installation.json")).unwrap(), before);
    assert!(!missing.exists());
}
#[cfg(all(
    feature = "installed-body-storage",
    target_os = "linux",
    target_arch = "x86_64"
))]
#[test]
fn configured_storage_reopens_same_private_root_and_refuses_substituted_directory() {
    let (manifest, state) = super::super::tests::fixture();
    install(&manifest, &state).unwrap();
    let root = tempfile::tempdir().unwrap();
    let source_path = state.join("storage.host.conduit");
    fs::write(&source_path, storage_source(root.path())).unwrap();
    configure(&state, Some(&source_path), false).unwrap();
    let installed = read_installation(&state.join("installation.json")).unwrap();
    let selected = installed.selected_host_configuration.unwrap();
    assert!(selected.storage_root.is_some());
    let mut host = StdHost::new();
    let ad = host.advertisement().clone();
    selected.attach_to_fresh_host(&mut host).unwrap();
    assert!(host.selected_body_storage().is_some());
    assert_eq!(host.advertisement().resources, ad.resources);
    assert_eq!(host.advertisement().capabilities, ad.capabilities);
    assert_eq!(host.advertisement().bases, ad.bases);
    let path = root.path().to_path_buf();
    let moved = path.with_extension("selected-moved");
    fs::rename(&path, &moved).unwrap();
    fs::create_dir(&path).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let mut fresh = StdHost::new();
    assert!(selected
        .attach_to_fresh_host(&mut fresh)
        .unwrap_err()
        .contains("identity changed"));
    assert!(fresh.selected_body_storage().is_none());
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    drop(host);
    fs::remove_dir_all(moved).unwrap();
}

#[cfg(all(
    feature = "installed-body-storage",
    feature = "installed-environmental-sources",
    target_os = "linux",
    target_arch = "x86_64"
))]
#[test]
fn storage_and_environmental_intent_coexist_without_network_effect_or_source_offers() {
    use conduit_host_make::{
        BaseInitialization, ConfigurationBase, ConfigurationDevice, ConfigurationScalar,
    };
    let root = tempfile::tempdir().unwrap();
    let mut configuration = parse_host_configuration_conduit(&storage_source(root.path())).unwrap();
    configuration.bases.push(ConfigurationBase {kind:"environment/source".into(),implementation:Some("linux/environmental-sources@1".into()),implementations:Vec::new(),initialization:Some(serde_json::from_value::<BaseInitialization>(serde_json::json!({"worker_executable":"/not-opened-by-inert-initialization","worker_sha256":"11".repeat(32),"timeout_milliseconds":31000})).unwrap())});
    let source_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bodies/environment/host/configurations/green.host.conduit")
        .canonicalize()
        .unwrap();
    configuration.devices.push(ConfigurationDevice::Network {
        id: "inert-green".into(),
        authority: "green.environment.invalid:8080".into(),
        endpoint: "127.0.0.1:9".into(),
        implementation: "environment/source@1".into(),
        configuration: std::collections::BTreeMap::from([
            (
                "source_configuration".into(),
                ConfigurationScalar::Text(source_path.to_str().unwrap().into()),
            ),
            (
                "firmware_sha256".into(),
                ConfigurationScalar::Text("11".repeat(32)),
            ),
        ]),
    });
    let source = canonical_host_configuration_conduit(&configuration).unwrap();
    let mut selection = Selection::from_source(&source, "std/x86_64/computer").unwrap();
    selection.storage_root = storage::select_identity(&selection.checked().unwrap()).unwrap();
    selection.validate().unwrap();
    let mut host = StdHost::new();
    let before = host.advertisement().clone();
    selection.attach_to_fresh_host(&mut host).unwrap();
    assert!(host.selected_body_storage().is_some());
    assert_eq!(host.advertisement().resources, before.resources);
    assert_eq!(host.advertisement().capabilities, before.capabilities);
    assert_eq!(host.advertisement().bases, before.bases);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
