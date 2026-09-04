//! Exact lowering from a resolved ConduitOS PROFILE to compile and linked-table inputs.

use conduit_host_fabrication::BuildManifest;

use super::ConduitosError;

mod http;
mod loongarch64;
pub(super) use loongarch64::lower_loongarch64_virt;

const NATIVE_PRESENTER: &str = "presenter/native-graphical@1";
const NATIVE_COMPOSITOR: &str = "compositor/native@1";
const PRESENT_OPERATION: &str = "conduit.host/present@1";
const SURFACE_CLASS: &str = "presentation/surface";
const SCANOUT_BASE: &str = "display/scanout";
const FRAMEBUFFER_DRIVER: &str = "display/linear-framebuffer@1";
const LINEAR_SERIAL_PRESENTER: &str = "presenter/linear-serial@1";
const SERIAL_TEXT_BASE: &str = "serial/text";
const PL011_DRIVER: &str = "conduitos/pl011@1";
const IA32_DEBUGCON_DRIVER: &str = "conduitos/ia32-debugcon@1";
const IA32_DEBUGCON_BASE: &str = "conduitos/ia32-debugcon-text";
const IA32_LINEAR_PRESENTER: &str = "presenter/ia32-linear-debugcon@1";
const RISCV64_SBI_DRIVER: &str = "conduitos/riscv64-sbi-console@1";
const RISCV64_SBI_BASE: &str = "conduitos/riscv64-sbi-console-text";
const RISCV64_LINEAR_PRESENTER: &str = "presenter/riscv64-linear-sbi-console@1";
const SCRIPTED_KEYBOARD_PROOF: &str = "profile-fragment/conduitos-scripted-keyboard-proof@1";
const HOTPLUG_PROOF: &str = "profile-fragment/conduitos-hotplug-proof@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TargetBuildInputs {
    pub cargo_features: Vec<&'static str>,
    pub implementations: u16,
    pub facilities: u16,
    pub resources: u16,
    pub bases: u16,
    pub drivers: u16,
    pub presenters: u16,
    pub proof_instrumentation: u16,
    pub presentation_surface_slots: u32,
    pub presentation_surface_bytes: u64,
}

pub(super) fn lower(manifest: &BuildManifest) -> Result<TargetBuildInputs, ConduitosError> {
    super::target_backend::select(&manifest.target)?.lower(manifest)
}

pub(super) fn lower_x86_64_pc(
    manifest: &BuildManifest,
) -> Result<TargetBuildInputs, ConduitosError> {
    let native = manifest
        .presenters
        .iter()
        .any(|item| item == NATIVE_PRESENTER);
    let presentation_surfaces = manifest
        .resource_budgets
        .iter()
        .filter(|item| item.class == SURFACE_CLASS)
        .collect::<Vec<_>>();
    if presentation_surfaces.len() > 1 {
        return Err(refusal(
            "profile-lowering-resource-ambiguous",
            format!(
                "expected at most one {SURFACE_CLASS} budget, found {}",
                presentation_surfaces.len()
            ),
        ));
    }
    let closure = [
        (
            PRESENT_OPERATION,
            manifest.host_operations.contains(&PRESENT_OPERATION.into()),
        ),
        (
            NATIVE_COMPOSITOR,
            manifest.facilities.contains(&NATIVE_COMPOSITOR.into()),
        ),
        (SURFACE_CLASS, !presentation_surfaces.is_empty()),
        (
            SCANOUT_BASE,
            manifest
                .base_selections
                .iter()
                .any(|item| item.kind == SCANOUT_BASE),
        ),
        (
            FRAMEBUFFER_DRIVER,
            manifest
                .driver_selections
                .iter()
                .any(|item| item.kind == FRAMEBUFFER_DRIVER),
        ),
    ];
    if native {
        if let Some((missing, _)) = closure.iter().find(|(_, present)| !present) {
            return Err(refusal(
                "profile-lowering-prerequisite-missing",
                format!("presenter:{NATIVE_PRESENTER} > {missing}"),
            ));
        }
    } else if let Some((leaked, _)) = closure.iter().find(|(_, present)| *present) {
        return Err(refusal(
            "headless-graphical-machinery-leaked",
            format!("headless PROFILE unexpectedly selected {leaked}"),
        ));
    }

    // The five portable nucleus implementations are the current non-optional
    // x86 product backbone. Optional inventory is derived only from the
    // resolved PROFILE closure above.
    let backbone = conduitos::fabrication::IMPL_TIME_TICK
        | conduitos::fabrication::IMPL_TICK_PRESENTATION
        | conduitos::fabrication::IMPL_TEXT_LITERAL
        | conduitos::fabrication::IMPL_TEXT_UPPER
        | conduitos::fabrication::IMPL_TEXT_PRESENTATION
        | conduitos::fabrication::IMPL_KEYBOARD
        | conduitos::fabrication::IMPL_PC_SPEAKER
        | conduitos::fabrication::IMPL_OPL2;
    let (presentation_surface_slots, presentation_surface_bytes) = presentation_surfaces
        .first()
        .map_or((0, 0), |item| (item.slots, item.bytes));
    let scripted_keyboard = manifest
        .profile_fragments
        .iter()
        .any(|item| item == SCRIPTED_KEYBOARD_PROOF);
    let hotplug = manifest
        .profile_fragments
        .iter()
        .any(|item| item == HOTPLUG_PROOF);
    let http = http::lower(manifest)?;
    if (scripted_keyboard || hotplug) && !native {
        return Err(refusal(
            "proof-profile-prerequisite-missing",
            "ConduitOS graphical proof instrumentation requires the native product closure".into(),
        ));
    }
    let mut cargo_features = native
        .then_some("native-compositor")
        .into_iter()
        .collect::<Vec<_>>();
    if hotplug {
        cargo_features.push("hotplug-proof");
    }
    if scripted_keyboard || hotplug {
        cargo_features.push("scripted-keyboard-proof");
    }
    if http.selected {
        cargo_features.push("native-http-client");
    }
    Ok(TargetBuildInputs {
        cargo_features,
        implementations: backbone
            | if native {
                conduitos::fabrication::IMPL_NATIVE_PRESENTER
            } else {
                0
            }
            | http.implementation,
        facilities: (if native {
            conduitos::fabrication::FACILITY_NATIVE_COMPOSITOR
        } else {
            0
        }) | http.facility,
        resources: (if native {
            conduitos::fabrication::RESOURCE_PRESENTATION_SURFACE
        } else {
            0
        }) | http.resource,
        bases: (if native {
            conduitos::fabrication::BASE_DISPLAY_SCANOUT
        } else {
            0
        }) | http.base,
        drivers: (if native {
            conduitos::fabrication::DRIVER_LINEAR_FRAMEBUFFER
        } else {
            0
        }) | http.driver,
        presenters: if native {
            conduitos::fabrication::PRESENTER_NATIVE_GRAPHICAL
        } else {
            0
        },
        proof_instrumentation: (if hotplug {
            conduitos::fabrication::PROOF_HOTPLUG
        } else {
            0
        }) | (if scripted_keyboard || hotplug {
            conduitos::fabrication::PROOF_SCRIPTED_KEYBOARD
        } else {
            0
        }),
        presentation_surface_slots,
        presentation_surface_bytes,
    })
}

pub(super) fn lower_aarch64_virt(
    manifest: &BuildManifest,
) -> Result<TargetBuildInputs, ConduitosError> {
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
    if manifest.host_operations.as_slice() != [PRESENT_OPERATION]
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

    let portable_backbone = conduitos::fabrication::IMPL_TIME_TICK
        | conduitos::fabrication::IMPL_TICK_PRESENTATION
        | conduitos::fabrication::IMPL_TEXT_LITERAL
        | conduitos::fabrication::IMPL_TEXT_UPPER
        | conduitos::fabrication::IMPL_TEXT_PRESENTATION;
    Ok(TargetBuildInputs {
        cargo_features: vec!["aarch64-product"],
        implementations: portable_backbone | conduitos::fabrication::IMPL_LINEAR_PRESENTER,
        facilities: 0,
        resources: 0,
        bases: conduitos::fabrication::BASE_SERIAL_TEXT,
        drivers: conduitos::fabrication::DRIVER_PL011_SERIAL,
        presenters: conduitos::fabrication::PRESENTER_LINEAR_SERIAL,
        proof_instrumentation: 0,
        presentation_surface_slots: 0,
        presentation_surface_bytes: 0,
    })
}

pub(super) fn lower_ia32_pc(manifest: &BuildManifest) -> Result<TargetBuildInputs, ConduitosError> {
    if manifest.presenters.as_slice() != [IA32_LINEAR_PRESENTER] {
        return Err(ia32_closure_refusal(
            "presenter",
            IA32_LINEAR_PRESENTER,
            &manifest.presenters,
        ));
    }
    let bases = manifest
        .base_selections
        .iter()
        .map(|item| item.kind.clone())
        .collect::<Vec<_>>();
    if bases.as_slice() != [IA32_DEBUGCON_BASE] {
        return Err(ia32_closure_refusal("base", IA32_DEBUGCON_BASE, &bases));
    }
    let drivers = manifest
        .driver_selections
        .iter()
        .map(|item| item.kind.clone())
        .collect::<Vec<_>>();
    if drivers.as_slice() != [IA32_DEBUGCON_DRIVER] {
        return Err(ia32_closure_refusal(
            "driver",
            IA32_DEBUGCON_DRIVER,
            &drivers,
        ));
    }
    if manifest.host_operations.as_slice() != [PRESENT_OPERATION]
        || !manifest.facilities.is_empty()
        || !manifest.resource_budgets.is_empty()
        || !manifest.profile_fragments.is_empty()
    {
        return Err(refusal(
            "ia32-product-closure-mismatch",
            "linear IA-32 product requires only present@1, with no facilities, resources, or proof instrumentation".into(),
        ));
    }
    if manifest.base_selections[0].driver != IA32_DEBUGCON_DRIVER {
        return Err(refusal(
            "ia32-serial-driver-mismatch",
            format!(
                "base {} selects {}, expected {IA32_DEBUGCON_DRIVER}",
                manifest.base_selections[0].kind, manifest.base_selections[0].driver
            ),
        ));
    }

    let portable_backbone = conduitos::fabrication::IMPL_TIME_TICK
        | conduitos::fabrication::IMPL_TICK_PRESENTATION
        | conduitos::fabrication::IMPL_TEXT_LITERAL
        | conduitos::fabrication::IMPL_TEXT_UPPER
        | conduitos::fabrication::IMPL_TEXT_PRESENTATION;
    Ok(TargetBuildInputs {
        cargo_features: vec!["ia32-product"],
        implementations: portable_backbone | conduitos::fabrication::IMPL_LINEAR_PRESENTER,
        facilities: 0,
        resources: 0,
        bases: conduitos::fabrication::BASE_SERIAL_TEXT,
        drivers: conduitos::fabrication::DRIVER_IA32_DEBUGCON_SERIAL,
        presenters: conduitos::fabrication::PRESENTER_LINEAR_SERIAL,
        proof_instrumentation: 0,
        presentation_surface_slots: 0,
        presentation_surface_bytes: 0,
    })
}

pub(super) fn lower_riscv64_virt(
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
    if presenters != [RISCV64_LINEAR_PRESENTER]
        || bases != [RISCV64_SBI_BASE]
        || drivers != [RISCV64_SBI_DRIVER]
        || manifest.base_selections[0].driver != RISCV64_SBI_DRIVER
        || manifest.host_operations.as_slice() != [PRESENT_OPERATION]
        || !manifest.facilities.is_empty()
        || !manifest.resource_budgets.is_empty()
        || !manifest.profile_fragments.is_empty()
    {
        return Err(refusal(
            "riscv64-product-closure-mismatch",
            "RISC-V64 product requires exactly its admitted SBI console presentation closure"
                .into(),
        ));
    }
    let portable = conduitos::fabrication::IMPL_TIME_TICK
        | conduitos::fabrication::IMPL_TICK_PRESENTATION
        | conduitos::fabrication::IMPL_TEXT_LITERAL
        | conduitos::fabrication::IMPL_TEXT_UPPER
        | conduitos::fabrication::IMPL_TEXT_PRESENTATION;
    Ok(TargetBuildInputs {
        cargo_features: vec!["riscv64-product"],
        implementations: portable | conduitos::fabrication::IMPL_LINEAR_PRESENTER,
        facilities: 0,
        resources: 0,
        bases: conduitos::fabrication::BASE_SERIAL_TEXT,
        drivers: conduitos::fabrication::DRIVER_RISCV64_SBI_CONSOLE,
        presenters: conduitos::fabrication::PRESENTER_LINEAR_SERIAL,
        proof_instrumentation: 0,
        presentation_surface_slots: 0,
        presentation_surface_bytes: 0,
    })
}

fn ia32_closure_refusal(class: &str, expected: &str, selected: &[String]) -> ConduitosError {
    refusal(
        "ia32-product-closure-mismatch",
        format!("expected exactly {class}:{expected}, found {selected:?}"),
    )
}

fn aarch64_closure_refusal(class: &str, expected: &str, selected: &[String]) -> ConduitosError {
    refusal(
        "aarch64-product-closure-mismatch",
        format!("expected exactly {class}:{expected}, found {selected:?}"),
    )
}

fn refusal(code: &'static str, detail: String) -> ConduitosError {
    ConduitosError::refusal(code, detail)
}

#[cfg(test)]
mod tests;
