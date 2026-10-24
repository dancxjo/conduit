use super::*;
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs,
};

fn manifest() -> BuildManifest {
    let packages = conduit_workspace_make::package_set();
    let profile = check_host_configuration(
        parse_host_configuration_conduit(include_str!(
            "../../../profiles/conduitos-loongarch64-virt.host.conduit"
        ))
        .unwrap(),
        &conduit_workspace_make::catalog(),
        &packages,
    )
    .unwrap()
    .into_profile();
    build_default_host_image(
        profile,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &BuildInputs {
            source_identity: "test-source".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0
    .manifest
}

#[test]
fn exact_closure_lowers_and_foreign_bindings_refuse() {
    let mut checked = manifest();
    let lowered = lower_loongarch64_virt(&checked).unwrap();
    assert_eq!(lowered.cargo_features, ["loongarch64-product"]);
    assert_eq!(lowered.drivers, conduitos::make::DRIVER_LOONGARCH64_UART);

    checked.presenters[0] = "presenter/riscv64-linear-sbi-console@1".into();
    assert!(lower_loongarch64_virt(&checked).is_err());
    checked.presenters[0] = LINEAR_PRESENTER.into();
    checked.driver_selections[0].kind = "conduitos/pl011@1".into();
    assert!(lower_loongarch64_virt(&checked).is_err());
}
