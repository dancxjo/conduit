//! Separate exact serial and firmware-scanout closures for the AArch64 product.
use super::*;

pub(crate) fn lower_aarch64_virt(
    manifest: &BuildManifest,
) -> Result<TargetBuildInputs, ConduitosError> {
    if manifest.presenters.as_slice() == [NATIVE_PRESENTER] {
        lower_graphical(manifest)
    } else {
        lower_linear(manifest)
    }
}

fn lower_graphical(manifest: &BuildManifest) -> Result<TargetBuildInputs, ConduitosError> {
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
    let surfaces = &manifest.resource_budgets;
    if bases.as_slice() != [SCANOUT_BASE]
        || drivers.as_slice() != [FRAMEBUFFER_DRIVER]
        || manifest.base_selections[0].driver != FRAMEBUFFER_DRIVER
        || manifest.host_calls.as_slice() != [PRESENT_OPERATION]
        || manifest.facilities.as_slice() != [NATIVE_COMPOSITOR]
        || surfaces.len() != 1
        || surfaces[0].class != SURFACE_CLASS
        || surfaces[0].slots == 0
        || surfaces[0].bytes == 0
        || !manifest.profile_fragments.is_empty()
    {
        return Err(refusal("aarch64-product-closure-mismatch", "native AArch64 requires exactly the admitted firmware scanout, native compositor and finite face surface; no input or proof profile is implied".into()));
    }
    Ok(TargetBuildInputs {
        cargo_features: vec!["aarch64-product", "native-compositor"],
        implementations: conduitos::make::IMPL_TIME_TICK
            | conduitos::make::IMPL_TICK_FACE
            | conduitos::make::IMPL_TEXT_LITERAL
            | conduitos::make::IMPL_TEXT_UPPER
            | conduitos::make::IMPL_TEXT_FACE
            | conduitos::make::IMPL_NATIVE_PRESENTER
            | conduitos::make::IMPL_USB_TRANSPORT,
        facilities: conduitos::make::FACILITY_NATIVE_COMPOSITOR,
        resources: conduitos::make::RESOURCE_FACE_SURFACE,
        bases: conduitos::make::BASE_DISPLAY_SCANOUT,
        drivers: conduitos::make::DRIVER_LINEAR_FRAMEBUFFER,
        presenters: conduitos::make::PRESENTER_NATIVE_GRAPHICAL,
        proof_instrumentation: 0,
        face_surface_slots: surfaces[0].slots,
        face_surface_bytes: surfaces[0].bytes,
    })
}

fn lower_linear(manifest: &BuildManifest) -> Result<TargetBuildInputs, ConduitosError> {
    if manifest.presenters.as_slice() != [LINEAR_SERIAL_PRESENTER] {
        return Err(aarch64_closure_refusal(
            "presenter",
            LINEAR_SERIAL_PRESENTER,
            &manifest.presenters,
        ));
    }
    let bases = manifest
        .base_selections
        .iter()
        .map(|item| item.kind.clone())
        .collect::<Vec<_>>();
    if bases.as_slice() != [SERIAL_TEXT_BASE] {
        return Err(aarch64_closure_refusal("base", SERIAL_TEXT_BASE, &bases));
    }
    let drivers = manifest
        .driver_selections
        .iter()
        .map(|item| item.kind.clone())
        .collect::<Vec<_>>();
    if drivers.as_slice() != [PL011_DRIVER] {
        return Err(aarch64_closure_refusal("driver", PL011_DRIVER, &drivers));
    }
    if manifest.host_calls.as_slice() != [PRESENT_OPERATION]
        || !manifest.facilities.is_empty()
        || !manifest.resource_budgets.is_empty()
        || !manifest.profile_fragments.is_empty()
    {
        return Err(refusal(
            "aarch64-product-closure-mismatch",
            "linear AArch64 product requires only present@1, with no facilities, resources, or proof instrumentation".into(),
        ));
    }
    let base = &manifest.base_selections[0];
    if base.driver != PL011_DRIVER {
        return Err(refusal(
            "aarch64-serial-driver-mismatch",
            format!(
                "base {} selects {}, expected {PL011_DRIVER}",
                base.kind, base.driver
            ),
        ));
    }

    let portable_backbone = conduitos::make::IMPL_TIME_TICK
        | conduitos::make::IMPL_TICK_FACE
        | conduitos::make::IMPL_TEXT_LITERAL
        | conduitos::make::IMPL_TEXT_UPPER
        | conduitos::make::IMPL_TEXT_FACE;
    Ok(TargetBuildInputs {
        cargo_features: vec!["aarch64-product"],
        implementations: portable_backbone
            | conduitos::make::IMPL_LINEAR_PRESENTER
            | conduitos::make::IMPL_USB_TRANSPORT,
        facilities: 0,
        resources: 0,
        bases: conduitos::make::BASE_SERIAL_TEXT,
        drivers: conduitos::make::DRIVER_PL011_SERIAL,
        presenters: conduitos::make::PRESENTER_LINEAR_SERIAL,
        proof_instrumentation: 0,
        face_surface_slots: 0,
        face_surface_bytes: 0,
    })
}

#[cfg(test)]
mod tests;
