use std::collections::BTreeMap;

use crate::*;

#[test]
fn checked_multihost_body_builds_distinct_body_bound_spores() {
    let body = checked_example();
    let spores =
        build_body_spores(&body, None, "git:test", &FabricationCatalog::canonical()).unwrap();
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
    assert_eq!(brainstem.manifest.architecture.features, ["line-usb-cdc"]);
    assert!(!brainstem
        .manifest
        .architecture
        .features
        .contains(&"wifi".into()));

    let one = build_body_spores(
        &body,
        Some("brainstem"),
        "git:test",
        &FabricationCatalog::canonical(),
    )
    .unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(
        one[0].manifest.architecture.architecture_package_id,
        "pico-rp2040@1"
    );
}

#[test]
fn body_binding_changes_spore_not_reusable_image_identity() {
    let body = checked_example();
    let first = build_body_spores(
        &body,
        Some("forebrain"),
        "git:test",
        &FabricationCatalog::canonical(),
    )
    .unwrap()
    .remove(0);
    let mut changed = parse_example();
    changed.body.id = "body:another".into();
    changed.hosts.retain(|host| host.name == "forebrain");
    let configurations = configurations_for(&changed);
    let changed =
        check_body_description(changed, &configurations, &FabricationCatalog::canonical()).unwrap();
    let second = build_body_spores(&changed, None, "git:test", &FabricationCatalog::canonical())
        .unwrap()
        .remove(0);
    assert_eq!(first.manifest.image_id, second.manifest.image_id);
    assert_ne!(first.manifest.spore_id, second.manifest.spore_id);
}

#[test]
fn validation_rejects_conflicts_incomplete_join_and_host_configuration_truth() {
    let mut duplicate = parse_example();
    duplicate.hosts[1].name = duplicate.hosts[0].name.clone();
    duplicate.hosts[1].part = duplicate.hosts[0].part.clone();
    let configurations = configurations_for(&duplicate);
    let errors =
        check_body_description(duplicate, &configurations, &FabricationCatalog::canonical())
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
        &FabricationCatalog::canonical(),
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
        &FabricationCatalog::canonical(),
    )
    .unwrap_err();
    assert!(errors.iter().any(|item| matches!(
        item,
        BodyDescriptionDiagnostic::InvalidHostConfiguration { .. }
    )));
    invalid.hosts[0].spore.output = SporeOutputKind::Uf2;
    let configurations = configurations_for(&invalid);
    let errors = check_body_description(invalid, &configurations, &FabricationCatalog::canonical())
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
    let package = architecture_package_for(&pico).unwrap();
    assert_eq!(
        package
            .derive(&pico, &SporeOutputKind::Uf2)
            .unwrap()
            .features,
        ["line-usb-cdc"]
    );

    let kernel = BaseSelection {
        id: "base/kernel".into(),
        kind: "kernel/signal".into(),
        driver: "esp32/kernel-signal@1".into(),
    };
    assert_eq!(
        derive_esp32_feature_closure(std::slice::from_ref(&kernel)).unwrap(),
        ["kernel-signal"]
    );
    let bluetooth = BaseSelection {
        id: "base/bluetooth".into(),
        kind: "line/bluetooth-le-gatt".into(),
        driver: "esp32/bluetooth-le-gatt@1".into(),
    };
    assert_eq!(
        derive_esp32_feature_closure(&[kernel, bluetooth]).unwrap(),
        ["bluetooth", "kernel-signal"]
    );
}

#[test]
fn deployment_receipt_is_separate_and_denies_runtime_claims() {
    let body = checked_example();
    let spore = build_body_spores(
        &body,
        Some("forebrain"),
        "git:test",
        &FabricationCatalog::canonical(),
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
fn descriptor_and_spore_model_dependency_graph_excludes_target_toolchains() {
    let output = std::process::Command::new("cargo")
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
    assert!(output.status.success());
    let graph = String::from_utf8(output.stdout).unwrap();
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
}

fn checked_example() -> CheckedBodyDescription {
    let description = parse_example();
    let configurations = configurations_for(&description);
    check_body_description(
        description,
        &configurations,
        &FabricationCatalog::canonical(),
    )
    .unwrap()
}

fn parse_example() -> BodyDescription {
    parse_body_description(include_str!("../../../profiles/bodies/pete-r1.body.toml")).unwrap()
}

fn configurations_for(description: &BodyDescription) -> BTreeMap<String, HostConfiguration> {
    description
        .hosts
        .iter()
        .map(|host| {
            let source = match host.name.as_str() {
                "forebrain" => include_str!(
                    "../../../profiles/host-configurations/linux-workstation.host.toml"
                ),
                "brainstem" => {
                    include_str!("../../../profiles/host-configurations/pico-w.host.toml")
                }
                "eyes" => {
                    include_str!("../../../profiles/host-configurations/browser-page.host.toml")
                }
                _ => include_str!(
                    "../../../profiles/host-configurations/linux-workstation.host.toml"
                ),
            };
            (
                host.configuration.clone(),
                parse_host_configuration(source).unwrap(),
            )
        })
        .collect()
}
