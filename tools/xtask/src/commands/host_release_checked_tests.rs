//! Synthetic release/command conformance; never compilation or physical proof.
use super::*;
use conduit_host_make::*;
use std::io::Cursor;

#[test]
fn target_sdk_paths_are_literal_link_inputs_without_runtime_rpath() {
    let (_, _, selection, _) = fixture();
    let target = NativeTarget::select(
        "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3",
        &selection,
    )
    .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "conduit SDK,{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    fs::create_dir(&directory).unwrap();
    let path = fs::canonicalize(&directory).unwrap();
    let args = target
        .arguments_with_libraries(&selection.features, std::slice::from_ref(&directory))
        .unwrap();
    assert_eq!(args[1], "rustc");
    assert!(args.contains(&format!("link-arg=-L{}", path.display())));
    assert!(args.contains(&format!("link-arg={}", path.display())));
    assert!(args.contains(&"link-arg=-rpath-link".into()));
    assert!(!args
        .iter()
        .any(|arg| arg == "link-arg=-rpath" || arg.contains("--allow-shlib-undefined")));
    assert!(target
        .arguments_with_libraries(&selection.features, &vec![directory.clone(); 17])
        .is_err());
    fs::remove_dir(&directory).unwrap();
    assert!(target
        .arguments_with_libraries(&selection.features, &[directory])
        .is_err());
}

fn configuration() -> CheckedHostConfiguration {
    let source = include_str!("../../../../targets/std/profiles/raspberry-pi-3-tick-workset.host.conduit")
        .replace("  capability =", "  base = {kind: \"display/scanout\", implementation: \"display/linear-framebuffer@1\", initialization: {backend: \"KMSDRM\", width: 640, height: 480}}\n  need = {id: \"display/main\", class: \"face/surface\", slots: 1, bytes: 1228800}\n  capability =");
    check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap()
}
fn fixture() -> (
    CheckedHostConfiguration,
    HostImage,
    MakeBuildSelection,
    HostReleaseManifest,
) {
    let checked = configuration();
    let packages = conduit_workspace_make::package_set();
    let selection = packages
        .derive_build_selection(checked.profile(), &SporeOutputKind::NativeBundle)
        .unwrap();
    let (image, _) = build_host_image(
        checked.profile().clone(),
        &conduit_workspace_make::catalog(),
        &packages,
        &SporeOutputKind::NativeBundle,
        &BuildInputs {
            source_identity: "fixture/source".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    let file = HostReleaseFile {
        path: "conduit-linux-aarch64".into(),
        bytes: 8,
        sha256: format!("sha256:{:x}", Sha256::digest(b"fixture!")),
        media_type: "application/vnd.conduit.host+executable".into(),
    };
    let manifest = HostReleaseManifest {
        schema: HOST_RELEASE_BUNDLE_SCHEMA.into(),
        target_id: image.manifest.target.clone(),
        make_package_id: selection.make_package_id.clone(),
        output: "native-bundle".into(),
        builder_adapter: selection.builder_adapter.clone(),
        deployment_adapter: selection.deployment_adapter.clone().unwrap(),
        source_identity: "fixture/source".into(),
        bundle_sha256: format!(
            "sha256:{}",
            bundle_digest(&[ReleaseFile {
                path: file.path.clone(),
                bytes: 8,
                sha256: file.sha256.clone(),
                media_type: "application/vnd.conduit.host+executable"
            }])
        ),
        reviewed_native_build: Some(ReviewedNativeBuild {
            schema: REVIEWED_NATIVE_BUILD_SCHEMA.into(),
            configuration_id: checked.configuration_id().into(),
            profile_id: image.manifest.profile_id.clone(),
            build_id: image.manifest.build_id.clone(),
            image_id: image.manifest.image_id.clone(),
            source_identity: "fixture/source".into(),
            selection: selection.clone(),
            executable_path: file.path.clone(),
            executable_sha256: file.sha256.clone(),
        }),
        files: vec![file],
        target_metadata: Default::default(),
    };
    (checked, image, selection, manifest)
}
fn verified(manifest: &HostReleaseManifest) -> VerifiedHostReleaseBytes {
    verify_host_release_bytes(
        &serde_json::to_vec(manifest).unwrap(),
        &ReleaseTargetProfile {
            target_id: &manifest.target_id,
            package_id: &manifest.make_package_id,
            output: "native-bundle",
            builder_adapter: &manifest.builder_adapter,
            deployment_adapter: &manifest.deployment_adapter,
        },
        "fixture/source",
        HostReleaseBounds {
            maximum_manifest_bytes: 65536,
            maximum_files: 2,
            maximum_file_bytes: 1024,
            maximum_bundle_bytes: 1024,
        },
        |_| Ok(Cursor::new(b"fixture!")),
        || false,
    )
    .unwrap()
}
#[test]
fn checked_features_drive_fixed_native_command_not_ambient_feature_text() {
    let (_, _, selection, _) = fixture();
    assert!(selection
        .features
        .contains(&"base-linear-framebuffer".into()));
    let target = NativeTarget::select(
        "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3",
        &selection,
    )
    .unwrap();
    let args = target.arguments(&selection.features);
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--features", &selection.features.join(",")]));
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--target", "aarch64-unknown-linux-gnu"]));
    assert!(args.contains(&"--no-default-features".into()));
    let mut unsupported = selection.clone();
    unsupported.features.push("unreviewed-feature".into());
    assert!(NativeTarget::select(
        "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3",
        &unsupported
    )
    .is_err());
    assert!(NativeTarget::select("std/aarch64/macOS", &selection).is_err());
}
#[test]
fn native_receipt_matches_exact_checked_closure_and_observed_payload() {
    let (checked, image, selection, manifest) = fixture();
    verify_native_release_build(&checked, &image, &verified(&manifest), &selection).unwrap();
    for field in [
        "configuration_id",
        "profile_id",
        "build_id",
        "image_id",
        "source_identity",
        "executable_sha256",
    ] {
        let mut value = serde_json::to_value(&manifest).unwrap();
        value["reviewed_native_build"][field] = "wrong".into();
        let changed = serde_json::from_value(value).unwrap();
        assert!(
            verify_native_release_build(&checked, &image, &verified(&changed), &selection).is_err(),
            "{field}"
        );
    }
    let mut missing = manifest.clone();
    missing.reviewed_native_build = None;
    assert!(
        verify_native_release_build(&checked, &image, &verified(&missing), &selection).is_err()
    );
    let mut changed = manifest;
    changed
        .reviewed_native_build
        .as_mut()
        .unwrap()
        .selection
        .features
        .clear();
    assert!(
        verify_native_release_build(&checked, &image, &verified(&changed), &selection).is_err()
    );
}

#[test]
fn pi5_non_cxx_make_selection_retains_exact_target_adapter_and_toolchain_checks() {
    let source =
        include_str!("../../../../targets/std/profiles/raspberry-pi-3-tick-workset.host.conduit")
            .replace(
                "raspberry-pi-3-model-b-plus-rev-1.3",
                "raspberry-pi-5-model-b-rev-1.1-8gb",
            );
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let selection = conduit_workspace_make::package_set()
        .derive_build_selection(checked.profile(), &SporeOutputKind::NativeBundle)
        .unwrap();
    let target = "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb";
    assert_eq!(
        selection.toolchain_identity,
        "rustc:1.98.1+aarch64-unknown-linux-gnu+gcc-aarch64-linux-gnu"
    );
    let native = NativeTarget::select(target, &selection).unwrap();
    assert_eq!(native.executable, "conduit-linux-aarch64");
    assert!(!native.cxx);
    let mut wrong = selection.clone();
    wrong.builder_adapter = "conduit-host-hosted/build-native@1".into();
    assert!(NativeTarget::select(target, &wrong).is_err());
    let mut wrong = selection.clone();
    wrong.toolchain_identity.push_str("+unreviewed");
    assert!(NativeTarget::select(target, &wrong).is_err());
}
