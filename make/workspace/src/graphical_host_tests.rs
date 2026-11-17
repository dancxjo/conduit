//! Construction intent only: no display is opened and no Ready offer is minted.
use conduit_host_make::{check_host_configuration, parse_host_configuration_conduit};

const PI3: &str = r#"host display-host {
  schema = 1
  target = {architecture: "aarch64", machine: "raspberry-pi-3-model-b-plus-rev-1.3", board: "raspberry-pi-3-model-b-plus-rev-1.3", os: "linux"}
  base = {kind: "display/scanout", implementation: "display/linear-framebuffer@1"}
  need = {id: "display/main", class: "face/surface", slots: 1, bytes: 2457600}
  need = {id: "display/memory", class: "conduit.resource/runtime-memory@1", slots: 1048576, bytes: 1048576}
  need = {id: "display/show", class: "face.resource/show@1", slots: 1, bytes: 1}
  limits = {static_memory_bytes: 16777216, heap_arena_bytes: 67108864, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}
}"#;

#[test]
fn pi3_graphical_construction_requires_explicit_scanout_and_surface() {
    let catalog = super::catalog();
    let packages = super::package_set();
    let check = |source: &str| {
        check_host_configuration(
            parse_host_configuration_conduit(source).unwrap(),
            &catalog,
            &packages,
        )
    };
    let checked = check(PI3).expect("Pi3 closes its selected graphical construction");
    let profile = checked.profile();
    assert!(profile
        .presenters
        .iter()
        .any(|presenter| { presenter.implementation == "presenter/native-graphical@1" }));
    assert!(profile
        .facilities
        .iter()
        .any(|facility| facility == "compositor/native@1"));
    assert!(profile
        .host_calls
        .iter()
        .any(|call| call == "conduit.host/present@1"));
    let zero_surface = PI3.replace("slots: 1", "slots: 0");
    assert!(
        check(&zero_surface).is_err(),
        "zero surface is not a graphical grant"
    );
    let foreign_driver = PI3.replace("display/linear-framebuffer@1", "browser/dom@1");
    assert!(
        check(&foreign_driver).is_err(),
        "unrelated driver cannot close scanout"
    );
}

#[test]
fn checked_native_presenter_closes_actual_back_implementations_into_image() {
    use conduit_host_make::{build_default_host_image, BuildInputs};
    let catalog = super::catalog();
    let packages = super::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(PI3).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let profile = checked.profile();
    for implementation in [
        "conduit.face/tee-kernel@1",
        "face/conduitos-surface-source@1",
        "face/renderer-conduitos-native@1",
        "face/conduitos-human-input@1",
    ] {
        assert!(
            profile
                .capabilities
                .iter()
                .any(|cap| cap.implementation == implementation),
            "missing actual Back {implementation}"
        );
    }
    assert!(profile
        .host_calls
        .contains(&"conduit.host/face-interaction@1".into()));
    assert!(!profile
        .resources
        .iter()
        .any(|pool| pool.class == "conduit.resource/conduitos-human-input@1"));
    let (image, _) = build_default_host_image(
        profile.clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "qualified-source-test".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    assert!(image
        .manifest
        .implementations
        .contains(&"face/renderer-conduitos-native@1".into()));
    assert!(image
        .manifest
        .host_calls
        .contains(&"conduit.host/face-interaction@1".into()));
    assert_eq!(
        image.manifest.resource_budgets,
        checked.configuration().resources
    );
}

#[test]
fn shipped_graphical_sources_declare_exact_native_mask_prerequisites() {
    use conduit_host_make::{build_default_host_image, BuildInputs};
    let catalog = super::catalog();
    let packages = super::package_set();
    for source in [
        include_str!("../../../targets/conduitos/profiles/conduitos-native.host.conduit"),
        include_str!("../../../targets/conduitos/profiles/conduitos-protocol-source.host.conduit"),
        include_str!(
            "../../../targets/conduitos/profiles/conduitos-aarch64-graphical.host.conduit"
        ),
        include_str!("../../../targets/conduitos/profiles/conduitos-ia32-graphical.host.conduit"),
        include_str!(
            "../../../targets/conduitos/profiles/conduitos-riscv64-graphical.host.conduit"
        ),
        include_str!(
            "../../../targets/conduitos/profiles/conduitos-loongarch64-graphical.host.conduit"
        ),
    ] {
        let check = |source: &str| {
            check_host_configuration(
                parse_host_configuration_conduit(source).unwrap(),
                &catalog,
                &packages,
            )
            .unwrap()
        };
        let checked = check(source);
        let profile = checked.profile();
        assert!(profile
            .presenters
            .iter()
            .any(|p| p.implementation == "presenter/native-graphical@1"));
        for (class, slots, bytes) in [
            ("face/surface", 4, 8388608),
            ("face.resource/show@1", 1, 1),
            ("conduit.resource/runtime-memory@1", 1048576, 1048576),
        ] {
            let pools = profile
                .resources
                .iter()
                .filter(|p| p.class == class)
                .collect::<Vec<_>>();
            assert_eq!(pools.len(), 1);
            assert_eq!((pools[0].slots, pools[0].bytes), (slots, bytes));
        }
        let (image, _) = build_default_host_image(
            profile.clone(),
            &catalog,
            &packages,
            &BuildInputs {
                source_identity: "shipped-profile-regression".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        assert!(image
            .manifest
            .presenters
            .contains(&"presenter/native-graphical@1".into()));
        let mut expected = checked.configuration().resources.clone();
        expected.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(image.manifest.resource_budgets, expected);
        for missing in ["face.resource/show@1", "conduit.resource/runtime-memory@1"] {
            let partial = source
                .lines()
                .filter(|line| !line.contains(missing))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(!check(&partial)
                .profile()
                .presenters
                .iter()
                .any(|p| p.implementation == "presenter/native-graphical@1"));
        }
    }
}

fn pi5_initialized_source() -> String {
    PI3.replace("raspberry-pi-3-model-b-plus-rev-1.3", "raspberry-pi-5-model-b-rev-1.1-8gb")
        .replace("implementation: \"display/linear-framebuffer@1\"}", "implementation: \"display/linear-framebuffer@1\", initialization: {backend: \"KMSDRM\", width: 1024, height: 600}}")
}
fn check_initialized(source: &str) -> conduit_host_make::CheckedHostConfiguration {
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &super::catalog(),
        &super::package_set(),
    )
    .unwrap()
}
#[test]
fn pi5_checked_scanout_and_mask_preserve_explicit_geometry_and_original_pools() {
    let checked = check_initialized(&pi5_initialized_source());
    let scanout = super::CheckedScanoutInitialization::from_configuration(&checked).unwrap();
    assert_eq!(
        (scanout.width(), scanout.height(), scanout.surface_bytes()),
        (1024, 600, 2_457_600)
    );
    assert_eq!(scanout.configuration_id(), checked.configuration_id());
    let mask = super::CheckedNativeMaskInitialization::from_configuration(&checked).unwrap();
    assert_eq!(mask.configuration_id(), checked.configuration_id());
    assert!(checked
        .profile()
        .presenters
        .iter()
        .any(|p| p.implementation == "presenter/native-graphical@1"));
    assert_eq!(
        checked
            .configuration()
            .resources
            .iter()
            .find(|p| p.class == "face/surface")
            .unwrap()
            .bytes,
        2_457_600
    );
}
#[test]
fn initialized_native_mask_refuses_missing_geometry_and_short_surface_without_enlargement() {
    let missing = check_initialized(&PI3.replace(
        "raspberry-pi-3-model-b-plus-rev-1.3",
        "raspberry-pi-5-model-b-rev-1.1-8gb",
    ));
    assert!(super::CheckedScanoutInitialization::from_configuration(&missing).is_err());
    let short =
        check_initialized(&pi5_initialized_source().replace("bytes: 2457600", "bytes: 2457599"));
    assert!(super::CheckedNativeMaskInitialization::from_configuration(&short).is_err());
    assert_eq!(
        short
            .configuration()
            .resources
            .iter()
            .find(|p| p.class == "face/surface")
            .unwrap()
            .bytes,
        2_457_599
    );
    let invalid =
        pi5_initialized_source().replace("backend: \"KMSDRM\"", "backend: \"unselected-backend\"");
    assert!(check_host_configuration(
        parse_host_configuration_conduit(&invalid).unwrap(),
        &super::catalog(),
        &super::package_set()
    )
    .is_err());
}
#[test]
fn pi5_display_and_checked_speaker_intents_compose_without_pcm_or_display_effects() {
    let source = pi5_initialized_source().replace("  limits =", "  base = {kind: \"audio/speaker\", implementation: \"std/kernel-audio-play-alsa-hw@1\", initialization: {executable_sha256: \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\", card_id: \"vc4hdmi1\", device: 0, base_identity: \"fixture-hdmi\"}}\n  limits =");
    let checked = check_initialized(&source);
    assert!(super::CheckedNativeMaskInitialization::from_configuration(&checked).is_ok());
    assert!(checked
        .resolved_bases()
        .iter()
        .any(|(kind, implementation)| kind == "audio/speaker"
            && implementation == "std/kernel-audio-play-alsa-hw@1"));
}

#[test]
fn pi5_display_and_pinned_whisper_capability_share_the_checked_catalog() {
    let source = pi5_initialized_source().replace("  limits =", "  base = {kind: \"speech/recognizer\", implementation: \"std/hosted-whisper-clip-speech@2\"}\n  capability = {kind: \"speech/recognize-clip\", pinned_implementation: \"std/hosted-whisper-clip-speech@2\"}\n  need = {id: \"speech/process\", class: \"conduit.resource/whisper-process-slot@1\", slots: 1, bytes: 1}\n  limits =");
    let checked = check_initialized(&source);
    assert!(super::CheckedNativeMaskInitialization::from_configuration(&checked).is_ok());
    assert!(checked.profile().capabilities.iter().any(|capability| {
        capability.kind == "speech/recognize-clip"
            && capability.implementation == "std/hosted-whisper-clip-speech@2"
    }));
    let foreign = source.replace(
        "pinned_implementation: \"std/hosted-whisper-clip-speech@2\"",
        "pinned_implementation: \"std/hosted-whisper-clip-speech@999\"",
    );
    assert!(check_host_configuration(
        parse_host_configuration_conduit(&foreign).unwrap(),
        &super::catalog(),
        &super::package_set(),
    )
    .is_err());
}
