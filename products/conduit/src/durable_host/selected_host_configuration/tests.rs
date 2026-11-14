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
        assert!(selected.require_image_initialization(true).is_err());
        let configuration_path = state.join("microphone.host.conduit");
        fs::write(&configuration_path, &selected.source).unwrap();
        configure(&state, Some(&configuration_path), false).unwrap();
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

#[test]
fn retained_scanout_intent_is_checked_but_never_treated_as_initialized_timer() {
    let source = SOURCE.replace("  limits =", r#"  base = {kind: "display/scanout", implementation: "display/linear-framebuffer@1", initialization: {backend: "KMSDRM", width: 1024, height: 600}}
  need = {id: "display/main", class: "face/surface", slots: 1, bytes: 2457600}
  limits ="#);
    let selected = Selection::from_source(&source, "std/x86_64/computer").unwrap();
    selected.validate().unwrap();
    let checked = selected.checked().unwrap();
    let parameters =
        conduit_workspace_make::CheckedScanoutInitialization::from_configuration(&checked).unwrap();
    assert_eq!(parameters.configuration_id(), selected.configuration_id());
    assert_eq!((parameters.width(), parameters.height()), (1024, 600));
    assert!(selected.require_timer_image_initialization().is_err());
    assert!(selected.require_image_initialization(true).is_ok());
    // Parsing and retention validation cannot upgrade intent into a Ready Base.
    let encoded = serde_json::to_string(&selected).unwrap();
    let reopened: Selection = serde_json::from_str(&encoded).unwrap();
    reopened.validate().unwrap();
    assert_eq!(reopened, selected);
    for invalid in [
        source.replace("KMSDRM", "dummy"),
        source.replace("width: 1024, ", ""),
        source.replace("width: 1024", "width: 4096"),
    ] {
        assert!(Selection::from_source(&invalid, "std/x86_64/computer").is_err());
    }
}

#[test]
fn seed_image_cannot_copy_source_ids_while_changing_equipment() {
    let checked = check(SOURCE).unwrap();
    let (image, _) = conduit_host_make::build_default_host_image(
        checked.profile().clone(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &conduit_host_make::BuildInputs {
            source_identity: "git:source-binding-test".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    check_seed_image(SOURCE, &image).unwrap();
    let mut forged = image.clone();
    forged.manifest.base_selections[0].driver = "unselected/driver@1".into();
    forged.payload.base_selections = forged.manifest.base_selections.clone();
    forged.manifest.image_id = format!(
        "image:{}",
        digest(&serde_json::to_vec(&forged.payload).unwrap())
    );
    // Even a self-consistent payload digest and copied Source/profile IDs do
    // not establish that its declared physical equipment matches the Source.
    let bytes = serde_json::to_vec(&forged).unwrap();
    conduit_host_make::verify_image_binding(&forged, &bytes).unwrap();
    assert!(check_seed_image(SOURCE, &forged)
        .unwrap_err()
        .contains("equipment and resources"));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn image_bound_release_update_validates_before_commit_and_retains_host_identity() {
    use super::super::{installation_configured, selected_model, selected_speech, selected_todo};
    let (manifest, state) = super::super::tests::fixture();
    let mut installed = install(&manifest, &state).unwrap();
    let checked = check(SOURCE).unwrap();
    let make_image = |source: &str| {
        conduit_host_make::build_default_host_image(
            checked.profile().clone(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
            &conduit_host_make::BuildInputs {
                source_identity: source.into(),
                toolchain_available: true,
            },
        )
        .unwrap()
        .0
    };
    let mut selection = Selection::from_source(SOURCE, "std/x86_64/computer").unwrap();
    selection.host_image = Some(make_image("commit:fixture"));
    installed.selected_host_configuration = Some(selection.clone());
    let installation_path = state.join("installation.json");
    write_json_atomic(&installation_path, &installed).unwrap();
    let source_path = state.join("same.host.conduit");
    fs::write(&source_path, SOURCE).unwrap();
    configure(&state, Some(&source_path), false).unwrap();
    assert_eq!(
        read_installation(&installation_path)
            .unwrap()
            .selected_host_configuration,
        Some(selection.clone())
    );
    let original = fs::read(&installation_path).unwrap();
    let service = fs::read(state.join("conduit-host.service")).unwrap();
    let mut release: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    release["source_identity"] = "commit:replacement".into();
    fs::write(&manifest, serde_json::to_vec(&release).unwrap()).unwrap();
    let apply = |image: Option<&Path>| {
        installation_configured::install_configured_with_image(
            &manifest,
            &state,
            selected_speech::Change::Preserve,
            selected_model::Change::Preserve,
            selected_todo::Change::Preserve,
            image,
        )
    };
    assert!(apply(None)
        .unwrap_err()
        .contains("checked replacement IMAGE"));
    let image_path = state.join("replacement-image.json");
    write_json_atomic(&image_path, &make_image("commit:wrong-release")).unwrap();
    assert!(apply(Some(&image_path)).is_err());
    let mut forged = make_image("commit:replacement");
    forged.manifest.image_id = "image:forged".into();
    write_json_atomic(&image_path, &forged).unwrap();
    assert!(apply(Some(&image_path)).is_err());
    assert_eq!(fs::read(&installation_path).unwrap(), original);
    assert_eq!(
        fs::read(state.join("conduit-host.service")).unwrap(),
        service
    );
    let mut oversized = installed.clone();
    oversized.product_executable = "x".repeat(64 * 1024);
    assert!(installation_configured::bounded_installation_bytes(&oversized).is_err());
    assert_eq!(fs::read(&installation_path).unwrap(), original);
    let replacement = make_image("commit:replacement");
    write_json_atomic(&image_path, &replacement).unwrap();
    let updated = apply(Some(&image_path)).unwrap();
    assert_eq!(updated.host_id, installed.host_id);
    assert_eq!(updated.release_source_identity, "commit:replacement");
    let selected = updated.selected_host_configuration.as_ref().unwrap();
    assert_eq!(selected.source, selection.source);
    assert_eq!(selected.configuration_id, selection.configuration_id);
    assert_eq!(selected.host_image.as_ref().unwrap(), &replacement);
    read_installation(&installation_path).unwrap();
    fs::remove_dir_all(state.parent().unwrap()).unwrap();
}
