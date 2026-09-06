//! Exact product closure for the LoongArch64 QEMU virt Host.

use conduit_host_fabrication::BuildManifest;

use super::{ConduitosError, TargetBuildInputs, PRESENT_OPERATION};

const UART_DRIVER: &str = "conduitos/loongarch64-uart@1";
const UART_BASE: &str = "conduitos/loongarch64-uart-text";
const LINEAR_PRESENTER: &str = "presenter/loongarch64-linear-uart@1";

pub(crate) fn lower_loongarch64_virt(
    manifest: &BuildManifest,
) -> Result<TargetBuildInputs, ConduitosError> {
    let presenters = manifest.presenters.as_slice();
    let bases = manifest
        .base_selections
        .iter()
        .map(|item| item.kind.clone())
        .collect::<Vec<_>>();
    let drivers = manifest
        .driver_selections
        .iter()
        .map(|item| item.kind.clone())
        .collect::<Vec<_>>();
    if presenters != [LINEAR_PRESENTER]
        || bases != [UART_BASE]
        || drivers != [UART_DRIVER]
        || manifest.base_selections[0].driver != UART_DRIVER
        || manifest.host_operations.as_slice() != [PRESENT_OPERATION]
        || !manifest.facilities.is_empty()
        || !manifest.resource_budgets.is_empty()
        || !manifest.profile_fragments.is_empty()
    {
        return Err(ConduitosError::refusal(
            "loongarch64-product-closure-mismatch",
            "LoongArch64 product requires exactly its admitted UART presentation closure",
        ));
    }
    let portable = conduitos::fabrication::IMPL_TIME_TICK
        | conduitos::fabrication::IMPL_TICK_PRESENTATION
        | conduitos::fabrication::IMPL_TEXT_LITERAL
        | conduitos::fabrication::IMPL_TEXT_UPPER
        | conduitos::fabrication::IMPL_TEXT_PRESENTATION;
    Ok(TargetBuildInputs {
        cargo_features: vec!["loongarch64-product"],
        implementations: portable | conduitos::fabrication::IMPL_LINEAR_PRESENTER,
        facilities: 0,
        resources: 0,
        bases: conduitos::fabrication::BASE_SERIAL_TEXT,
        drivers: conduitos::fabrication::DRIVER_LOONGARCH64_UART,
        presenters: conduitos::fabrication::PRESENTER_LINEAR_SERIAL,
        proof_instrumentation: 0,
        presentation_surface_slots: 0,
        presentation_surface_bytes: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_fabrication::{build_default_host_image, BuildInputs, HostProfile};

    fn manifest() -> BuildManifest {
        let profile: HostProfile = serde_json::from_str(include_str!(
            "../../../profiles/conduitos-loongarch64-headless.profile.json"
        ))
        .unwrap();
        build_default_host_image(
            profile,
            &conduit_workspace_fabrication::catalog(),
            &conduit_workspace_fabrication::package_set(),
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
        assert_eq!(
            lowered.drivers,
            conduitos::fabrication::DRIVER_LOONGARCH64_UART
        );

        checked.presenters[0] = "presenter/riscv64-linear-sbi-console@1".into();
        assert!(lower_loongarch64_virt(&checked).is_err());
        checked.presenters[0] = LINEAR_PRESENTER.into();
        checked.driver_selections[0].kind = "conduitos/pl011@1".into();
        assert!(lower_loongarch64_virt(&checked).is_err());
    }
}
