//! Construction intent only: no display is opened and no Ready offer is minted.
use conduit_host_make::{check_host_configuration, parse_host_configuration_conduit};

const PI3: &str = r#"host display-host {
  schema = 1
  target = {architecture: "aarch64", machine: "raspberry-pi-3-model-b-plus-rev-1.3", board: "raspberry-pi-3-model-b-plus-rev-1.3", os: "linux"}
  base = {kind: "display/scanout", implementation: "display/linear-framebuffer@1"}
  need = {id: "display/main", class: "face/surface", slots: 1, bytes: 2457600}
  need = {id: "display/show", class: "face.resource/show@1", slots: 1, bytes: 1}
  need = {id: "display/memory", class: "conduit.resource/runtime-memory@1", slots: 1048576, bytes: 1048576}
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
fn direct_display_initialization_is_explicit_finite_and_source_bound() {
    use conduit_linear_framebuffer_make::CheckedScanoutInitialization;
    let catalog = super::catalog();
    let packages = super::package_set();
    let check = |source: &str| {
        check_host_configuration(
            parse_host_configuration_conduit(source).unwrap(),
            &catalog,
            &packages,
        )
    };
    let source = PI3.replace(
        "implementation: \"display/linear-framebuffer@1\"}",
        "implementation: \"display/linear-framebuffer@1\", initialization: {backend: \"KMSDRM\", width: 1024, height: 600}}",
    );
    let checked = check(&source).unwrap();
    let intent = CheckedScanoutInitialization::from_configuration(&checked).unwrap();
    assert_eq!(intent.configuration_id(), checked.configuration_id());
    assert_eq!((intent.width(), intent.height()), (1024, 600));
    assert_eq!(intent.surface_bytes(), 2_457_600);
    let changed = check(&source.replace("width: 1024", "width: 1280")).unwrap();
    assert_ne!(checked.configuration_id(), changed.configuration_id());
    let canonical =
        conduit_host_make::canonical_host_configuration_conduit(checked.configuration()).unwrap();
    assert_eq!(
        check(&canonical).unwrap().configuration_id(),
        checked.configuration_id()
    );
    for invalid in [
        source.replace("KMSDRM", "dummy"),
        source.replace("width: 1024, ", ""),
        source.replace("width: 1024", "width: 0"),
        source.replace("height: 600", "height: 4097"),
        source.replace("height: 600", "unknown: 600"),
    ] {
        assert!(check(&invalid).is_err());
    }
    // Independent dimension limits do not bypass the raster's total byte cap.
    let oversized = check(&source.replace("width: 1024", "width: 4096")).unwrap();
    assert!(CheckedScanoutInitialization::from_configuration(&oversized).is_err());
    let uninitialized = check(PI3).unwrap();
    assert!(CheckedScanoutInitialization::from_configuration(&uninitialized).is_err());
}

#[test]
fn native_mask_intent_requires_explicit_show_and_byte_backed_memory() {
    use super::CheckedNativeMaskInitialization;
    let check = |source: &str| {
        check_host_configuration(
            parse_host_configuration_conduit(source).unwrap(),
            &super::catalog(),
            &super::package_set(),
        )
    };
    let source=PI3.replace("implementation: \"display/linear-framebuffer@1\"}", "implementation: \"display/linear-framebuffer@1\", initialization: {backend: \"KMSDRM\", width: 1024, height: 600}}");
    let checked = check(&source).unwrap();
    let intent = CheckedNativeMaskInitialization::from_configuration(&checked).unwrap();
    assert_eq!(intent.configuration_id(), checked.configuration_id());
    assert_eq!(intent.show().id, "display/show");
    assert_eq!(intent.memory().slots, 1048576);
    assert_eq!(intent.scanout().surface_bytes(), intent.surface().bytes);
    let canonical =
        conduit_host_make::canonical_host_configuration_conduit(checked.configuration()).unwrap();
    assert_eq!(
        CheckedNativeMaskInitialization::from_configuration(&check(&canonical).unwrap())
            .unwrap()
            .configuration_id(),
        intent.configuration_id()
    );
    for class in ["face.resource/show@1", "conduit.resource/runtime-memory@1"] {
        let missing = source
            .lines()
            .filter(|line| !line.contains(class))
            .collect::<Vec<_>>()
            .join("\n");
        let bare = check(&missing).unwrap();
        assert!(!bare
            .profile()
            .presenters
            .iter()
            .any(|presenter| presenter.implementation == "presenter/native-graphical@1"));
        assert!(CheckedNativeMaskInitialization::from_configuration(&bare).is_err());
        let duplicate=source.replace("  limits =", &format!("  need = {{id: \"duplicate\", class: \"{class}\", slots: 1, bytes: 1}}\n  limits ="));
        assert!(
            CheckedNativeMaskInitialization::from_configuration(&check(&duplicate).unwrap())
                .is_err()
        );
    }
    let insufficient = check(&source.replace(
        "slots: 1048576, bytes: 1048576",
        "slots: 1048576, bytes: 1048575",
    ))
    .unwrap();
    assert!(CheckedNativeMaskInitialization::from_configuration(&insufficient).is_err());
    let wrong_show_capacity = check(&source.replace(
        "class: \"face.resource/show@1\", slots: 1",
        "class: \"face.resource/show@1\", slots: 2",
    ))
    .unwrap();
    assert!(CheckedNativeMaskInitialization::from_configuration(&wrong_show_capacity).is_err());
    let bare_physical = source
        .lines()
        .filter(|line| {
            !line.contains("face.resource/show@1")
                && !line.contains("conduit.resource/runtime-memory@1")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!check(&bare_physical)
        .unwrap()
        .profile()
        .presenters
        .iter()
        .any(|presenter| presenter.implementation == "presenter/native-graphical@1"));
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
    let mut declared_resources = checked.configuration().resources.clone();
    declared_resources.sort_by(|left, right| left.id.cmp(&right.id));
    assert_eq!(image.manifest.resource_budgets, declared_resources);
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
