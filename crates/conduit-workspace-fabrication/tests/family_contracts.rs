use conduit_host_fabrication::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, PostBuildAction, SporeOutputKind,
};

fn build_configuration(source: &str) -> conduit_host_fabrication::HostImage {
    let packages = conduit_workspace_fabrication::package_set();
    let catalog = conduit_workspace_fabrication::catalog();
    let profile = check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap()
    .into_profile();
    build_default_host_image(
        profile,
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "git:family-contract-proof".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0
}

#[test]
fn native_and_browser_are_not_flash_shaped() {
    let workstation = build_configuration(include_str!(
        "../../../profiles/host-configurations/linux-workstation.host.conduit"
    ));
    let server = build_configuration(include_str!(
        "../../../profiles/host-configurations/linux-server.host.conduit"
    ));
    let browser = build_configuration(include_str!(
        "../../../profiles/host-configurations/browser-page.host.conduit"
    ));

    assert_eq!(workstation.manifest.output, SporeOutputKind::NativeBundle);
    assert_eq!(server.manifest.output, SporeOutputKind::NativeBundle);
    assert_eq!(
        workstation.manifest.post_build_actions,
        [PostBuildAction::Launch]
    );
    assert_eq!(
        server.manifest.post_build_actions,
        [PostBuildAction::Launch]
    );
    assert_ne!(
        workstation.manifest.base_selections, server.manifest.base_selections,
        "one hosted package must preserve narrower construction choices"
    );
    assert_eq!(browser.manifest.output, SporeOutputKind::BrowserBundle);
    assert_eq!(
        browser.manifest.post_build_actions,
        [PostBuildAction::Load, PostBuildAction::Launch]
    );
    for image in [&workstation, &server, &browser] {
        assert!(!image
            .manifest
            .post_build_actions
            .contains(&PostBuildAction::Flash));
        assert!(!image
            .manifest
            .post_build_actions
            .contains(&PostBuildAction::Boot));
    }
}

#[test]
fn exact_conduitos_and_raspberry_pi_families_have_one_owner_each() {
    let packages = conduit_workspace_fabrication::package_set();
    let conduitos = [
        "conduitos/x86_64/pc",
        "conduitos/ia32/pc",
        "conduitos/aarch64/virt",
        "conduitos/riscv64/virt",
        "conduitos/loongarch64/virt",
    ];
    for target in conduitos {
        let descriptor = packages.target_descriptor(target).unwrap();
        assert_eq!(
            packages.anchor_for_target(target).unwrap().package_id,
            "conduitos-image@1"
        );
        assert_eq!(descriptor.default_output, SporeOutputKind::DiskImage);
        assert_eq!(descriptor.post_build_actions, [PostBuildAction::Boot]);
    }

    for target in [
        "conduitos/armv6/raspberry-pi-model-b-plus-v1.2",
        "conduitos/armv6/raspberry-pi-zero-v1",
    ] {
        let descriptor = packages.target_descriptor(target).unwrap();
        assert_eq!(
            packages.anchor_for_target(target).unwrap().package_id,
            "conduit-host-raspberry-pi@1"
        );
        assert_eq!(descriptor.default_output, SporeOutputKind::SdImage);
        assert_eq!(
            descriptor.post_build_actions,
            [PostBuildAction::Flash, PostBuildAction::Boot]
        );
    }
}
