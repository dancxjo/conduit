use std::collections::BTreeMap;

use crate::*;
use conduit_host_fabrication::*;
use conduit_workspace_fabrication::{catalog as test_catalog, package_set as test_package_set};

#[test]
fn checked_multihost_body_builds_distinct_body_bound_spores() {
    let body = checked_example();
    let spores = build_body_spores(
        &body,
        None,
        "git:test",
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap();
    assert_eq!(spores.len(), 3);
    assert!(spores
        .iter()
        .all(|spore| spore.manifest.body_id == "body:pete-r1"));
    assert!(spores.iter().all(|spore| !spore
        .image_bytes
        .windows(6)
        .any(|window| window == b"HostId")));
    let brainstem = spores
        .iter()
        .find(|item| item.manifest.host_entry_name == "brainstem")
        .unwrap();
    assert_eq!(brainstem.manifest.fabrication.features, ["line-usb-cdc"]);
    assert!(!brainstem
        .manifest
        .fabrication
        .features
        .contains(&"wifi".into()));

    let one = build_body_spores(
        &body,
        Some("brainstem"),
        "git:test",
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(
        one[0].manifest.fabrication.fabrication_package_id,
        "conduit-host-rp2040@1"
    );
}

#[test]
fn exact_lifecycle_body_identity_is_a_valid_fabrication_binding() {
    let mut description = parse_example();
    description.body.id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into();
    let configurations = configurations_for(&description);
    assert!(check_body_description(
        description,
        &configurations,
        &test_catalog(),
        &test_package_set(),
    )
    .is_ok());

    let mut invalid = parse_example();
    invalid.body.id = "not-a-body".into();
    let configurations = configurations_for(&invalid);
    assert!(matches!(
        check_body_description(
            invalid,
            &configurations,
            &test_catalog(),
            &test_package_set(),
        )
        .unwrap_err()
        .as_slice(),
        [BodyDescriptionDiagnostic::InvalidBodyId]
    ));
}

#[test]
fn body_binding_changes_spore_not_reusable_image_identity() {
    let body = checked_example();
    let first = build_body_spores(
        &body,
        Some("forebrain"),
        "git:test",
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap()
    .remove(0);
    let mut changed = parse_example();
    changed.body.id = "body:another".into();
    changed.hosts.retain(|host| host.name == "forebrain");
    let configurations = configurations_for(&changed);
    let changed = check_body_description(
        changed,
        &configurations,
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap();
    let second = build_body_spores(
        &changed,
        None,
        "git:test",
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap()
    .remove(0);
    assert_eq!(first.manifest.image_id, second.manifest.image_id);
    assert_ne!(first.manifest.spore_id, second.manifest.spore_id);
}

#[test]
fn prebuilt_image_seals_a_fresh_spore_without_changing_image_truth() {
    let body = checked_example();
    let built = build_body_spores(
        &body,
        Some("brainstem"),
        "git:image-build",
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap()
    .remove(0);
    let sealed = seal_prebuilt_body_spore(
        &body,
        "brainstem",
        "body:browser-birth/1",
        &built.image,
        &built.image_bytes,
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap();

    assert_eq!(sealed.image, built.image);
    assert_eq!(sealed.image_bytes, built.image_bytes);
    assert_eq!(sealed.manifest.image_id, built.manifest.image_id);
    assert_ne!(sealed.manifest.spore_id, built.manifest.spore_id);
    assert_eq!(sealed.manifest.source_identity, "body:browser-birth/1");
    assert_eq!(
        sealed.manifest.binding,
        SporeBinding::Prejoined {
            part_id: "part:brainstem".into()
        }
    );
}

#[test]
fn prebuilt_spore_refuses_wrong_bytes_image_and_missing_identity() {
    let body = checked_example();
    let built = build_body_spores(
        &body,
        Some("brainstem"),
        "git:image-build",
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap()
    .remove(0);
    let mut wrong_bytes = built.image_bytes.clone();
    wrong_bytes[0] ^= 1;
    assert!(matches!(
        seal_prebuilt_body_spore(
            &body,
            "brainstem",
            "body:browser-birth/1",
            &built.image,
            &wrong_bytes,
            &test_catalog(),
            &test_package_set(),
        ),
        Err(BodyBuildDiagnostic::SelectedImageMismatch { .. })
    ));

    let mut wrong_image = built.image.clone();
    wrong_image.manifest.image_id.push_str("-stale");
    assert!(matches!(
        seal_prebuilt_body_spore(
            &body,
            "brainstem",
            "body:browser-birth/1",
            &wrong_image,
            &built.image_bytes,
            &test_catalog(),
            &test_package_set(),
        ),
        Err(BodyBuildDiagnostic::SelectedImageMismatch { .. })
    ));
    assert_eq!(
        seal_prebuilt_body_spore(
            &body,
            "brainstem",
            " ",
            &built.image,
            &built.image_bytes,
            &test_catalog(),
            &test_package_set(),
        ),
        Err(BodyBuildDiagnostic::SourceIdentityMissing)
    );
    assert!(matches!(
        seal_prebuilt_body_spore(
            &body,
            "missing",
            "body:browser-birth/1",
            &built.image,
            &built.image_bytes,
            &test_catalog(),
            &test_package_set(),
        ),
        Err(BodyBuildDiagnostic::UnknownHost { .. })
    ));
}

#[test]
fn validation_rejects_conflicts_incomplete_join_and_host_configuration_truth() {
    let mut duplicate = parse_example();
    duplicate.hosts[1].name = duplicate.hosts[0].name.clone();
    duplicate.hosts[1].part = duplicate.hosts[0].part.clone();
    let configurations = configurations_for(&duplicate);
    let errors = check_body_description(
        duplicate,
        &configurations,
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap_err();
    assert!(errors
        .iter()
        .any(|item| matches!(item, BodyDescriptionDiagnostic::DuplicateHost { .. })));
    assert!(errors
        .iter()
        .any(|item| matches!(item, BodyDescriptionDiagnostic::DuplicatePart { .. })));

    let mut incomplete = parse_example();
    incomplete.hosts[0].part = None;
    incomplete.hosts[2].spore.invitation = None;
    let configurations = configurations_for(&incomplete);
    let errors = check_body_description(
        incomplete,
        &configurations,
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap_err();
    assert!(errors
        .iter()
        .any(|item| matches!(item, BodyDescriptionDiagnostic::MissingPrejoinedPart { .. })));
    assert!(errors
        .iter()
        .any(|item| matches!(item, BodyDescriptionDiagnostic::MissingInvitation { .. })));

    let mut invalid = parse_example();
    let mut configurations = configurations_for(&invalid);
    configurations
        .get_mut(&invalid.hosts[0].configuration)
        .unwrap()
        .limits
        .queue_items = 0;
    let errors = check_body_description(
        invalid.clone(),
        &configurations,
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap_err();
    assert!(errors.iter().any(|item| matches!(
        item,
        BodyDescriptionDiagnostic::InvalidHostConfiguration { .. }
    )));
    invalid.hosts[0].spore.output = SporeOutputKind::Uf2;
    let configurations = configurations_for(&invalid);
    let errors = check_body_description(
        invalid,
        &configurations,
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap_err();
    assert!(errors
        .iter()
        .any(|item| matches!(item, BodyDescriptionDiagnostic::IncompatibleOutput { .. })));
}

#[test]
fn pico_and_esp32_packages_derive_only_selected_base_features() {
    let pico = checked_example()
        .hosts()
        .iter()
        .find(|host| host.description.name == "brainstem")
        .unwrap()
        .configuration
        .profile()
        .clone();
    let packages = test_package_set();
    assert_eq!(
        packages
            .derive_build_selection(&pico, &SporeOutputKind::Uf2)
            .unwrap()
            .features,
        ["line-usb-cdc"]
    );

    let kernel = BaseSelection {
        id: "base/kernel".into(),
        kind: "kernel/signal".into(),
        driver: "esp32/kernel-signal@1".into(),
    };
    let mut esp32_profile = pico.clone();
    esp32_profile.target.family = "esp32".into();
    esp32_profile.target.architecture = "xtensa-lx6".into();
    esp32_profile.target.machine = "hw-463-esp-wroom-32".into();
    esp32_profile.bases = vec![kernel.clone()];
    assert_eq!(
        packages
            .derive_build_selection(&esp32_profile, &SporeOutputKind::Esp32Image)
            .unwrap()
            .features,
        ["kernel-signal"]
    );
    let bluetooth = BaseSelection {
        id: "base/bluetooth".into(),
        kind: "line/bluetooth-le-gatt".into(),
        driver: "esp32/bluetooth-le-gatt@1".into(),
    };
    esp32_profile.bases = vec![kernel, bluetooth];
    let esp32 = packages
        .derive_build_selection(&esp32_profile, &SporeOutputKind::Esp32Image)
        .unwrap();
    assert_eq!(esp32.features, ["bluetooth", "kernel-signal"]);
    assert_eq!(esp32.fabrication_package_revision, 1);
    assert_eq!(esp32.builder_adapter, "conduit-host-esp32/build-image@1");
}

#[test]
fn deployment_receipt_is_separate_and_denies_runtime_claims() {
    let body = checked_example();
    let spore = build_body_spores(
        &body,
        Some("forebrain"),
        "git:test",
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap()
    .remove(0);
    let receipt = deployment_receipt(&body, &spore, DeploymentDisposition::Prepared).unwrap();
    assert_eq!(receipt.disposition, DeploymentDisposition::Prepared);
    assert_eq!(
        receipt.does_not_prove,
        ["boot", "join", "presence", "runtime-readiness"]
    );
}

#[test]
fn body_fabrication_depends_one_way_on_host_fabrication_without_target_toolchains() {
    let output = std::process::Command::new("cargo")
        .args([
            "tree",
            "--manifest-path",
            "../../Cargo.toml",
            "-p",
            "conduit-body-fabrication",
            "--prefix",
            "none",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let graph = String::from_utf8(output.stdout).unwrap();
    assert!(graph.contains("conduit-host-fabrication"));
    for forbidden in [
        "arpabet_cmudict",
        "esp-idf",
        "embassy-rp",
        "wasm-bindgen-cli",
    ] {
        assert!(
            !graph.contains(forbidden),
            "unexpected heavyweight dependency: {forbidden}"
        );
    }

    let host_graph = std::process::Command::new("cargo")
        .args([
            "tree",
            "--manifest-path",
            "../../Cargo.toml",
            "-p",
            "conduit-host-fabrication",
            "--prefix",
            "none",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(host_graph.status.success());
    assert!(!String::from_utf8(host_graph.stdout)
        .unwrap()
        .contains("conduit-body-fabrication"));
}

fn checked_example() -> CheckedBodyDescription {
    let description = parse_example();
    let configurations = configurations_for(&description);
    check_body_description(
        description,
        &configurations,
        &test_catalog(),
        &test_package_set(),
    )
    .unwrap()
}

fn parse_example() -> BodyDescription {
    parse_body_description_conduit(include_str!(
        "../../../bodies/pete/profiles/pete-r1.body.conduit"
    ))
    .unwrap()
}

fn configurations_for(description: &BodyDescription) -> BTreeMap<String, HostConfiguration> {
    description
        .hosts
        .iter()
        .map(|host| {
            let source = match host.name.as_str() {
                "forebrain" => {
                    include_str!("../../../targets/std/profiles/linux-computer.host.conduit")
                }
                "brainstem" => {
                    include_str!("../../../targets/rp2040/profiles/pico-w.host.conduit")
                }
                "eyes" => {
                    include_str!("../../../targets/browser/profiles/browser-page.host.conduit")
                }
                _ => include_str!("../../../targets/std/profiles/linux-computer.host.conduit"),
            };
            (
                host.configuration.clone(),
                parse_host_configuration_conduit(source).unwrap(),
            )
        })
        .collect()
}
