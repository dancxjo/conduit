//! Exact IA-32 serial plus optional firmware-framebuffer product closure.
use super::{
    BuildManifest, ConduitosError, FRAMEBUFFER_DRIVER, IA32_DEBUGCON_BASE, IA32_DEBUGCON_DRIVER,
    IA32_LINEAR_PRESENTER, NATIVE_COMPOSITOR, NATIVE_PRESENTER, PRESENT_OPERATION, SCANOUT_BASE,
    SURFACE_CLASS, TargetBuildInputs, refusal,
};

pub(crate) fn lower_ia32_pc(manifest: &BuildManifest) -> Result<TargetBuildInputs, ConduitosError> {
    let graphical = manifest.presenters.iter().any(|p| p == NATIVE_PRESENTER);
    let mut presenters = vec![IA32_LINEAR_PRESENTER];
    let mut bases = vec![IA32_DEBUGCON_BASE];
    let mut drivers = vec![IA32_DEBUGCON_DRIVER];
    let mut facilities = Vec::new();
    if graphical {
        presenters.push(NATIVE_PRESENTER);
        bases.push(SCANOUT_BASE);
        drivers.push(FRAMEBUFFER_DRIVER);
        facilities.push(NATIVE_COMPOSITOR);
    }
    exact("presenter", &presenters, &manifest.presenters)?;
    exact(
        "base",
        &bases,
        &manifest
            .base_selections
            .iter()
            .map(|b| b.kind.clone())
            .collect::<Vec<_>>(),
    )?;
    exact(
        "driver",
        &drivers,
        &manifest
            .driver_selections
            .iter()
            .map(|d| d.kind.clone())
            .collect::<Vec<_>>(),
    )?;
    exact("facility", &facilities, &manifest.facilities)?;
    exact("host-call", &[PRESENT_OPERATION], &manifest.host_calls)?;
    if !manifest.profile_fragments.is_empty() {
        return Err(refusal(
            "ia32-product-closure-mismatch",
            "ordinary IA-32 product cannot select proof instrumentation".into(),
        ));
    }
    for base in &manifest.base_selections {
        let expected = if base.kind == IA32_DEBUGCON_BASE {
            IA32_DEBUGCON_DRIVER
        } else {
            FRAMEBUFFER_DRIVER
        };
        if base.driver != expected {
            return Err(refusal(
                if base.kind == IA32_DEBUGCON_BASE {
                    "ia32-serial-driver-mismatch"
                } else {
                    "ia32-scanout-driver-mismatch"
                },
                format!(
                    "base {} selects {}, expected {expected}",
                    base.kind, base.driver
                ),
            ));
        }
    }
    let (surface_slots, surface_bytes) = match manifest.resource_budgets.as_slice() {
        [] if !graphical => (0, 0),
        [surface]
            if graphical
                && surface.class == SURFACE_CLASS
                && surface.slots != 0
                && surface.bytes != 0 =>
        {
            (surface.slots, surface.bytes)
        }
        _ => return Err(refusal(
            "ia32-product-closure-mismatch",
            "graphical IA-32 requires exactly one finite face/surface budget; serial selects none"
                .into(),
        )),
    };
    let mut features = vec!["ia32-product"];
    if graphical {
        features.push("native-compositor-core");
    }
    let portable_backbone = conduitos::make::IMPL_TIME_TICK
        | conduitos::make::IMPL_TICK_FACE
        | conduitos::make::IMPL_TEXT_LITERAL
        | conduitos::make::IMPL_TEXT_UPPER
        | conduitos::make::IMPL_TEXT_FACE;
    Ok(TargetBuildInputs {
        cargo_features: features,
        implementations: portable_backbone
            | conduitos::make::IMPL_LINEAR_PRESENTER
            | if graphical {
                conduitos::make::IMPL_NATIVE_PRESENTER
            } else {
                0
            },
        facilities: if graphical {
            conduitos::make::FACILITY_NATIVE_COMPOSITOR
        } else {
            0
        },
        resources: if graphical {
            conduitos::make::RESOURCE_FACE_SURFACE
        } else {
            0
        },
        bases: conduitos::make::BASE_SERIAL_TEXT
            | if graphical {
                conduitos::make::BASE_DISPLAY_SCANOUT
            } else {
                0
            },
        drivers: conduitos::make::DRIVER_IA32_DEBUGCON_SERIAL
            | if graphical {
                conduitos::make::DRIVER_LINEAR_FRAMEBUFFER
            } else {
                0
            },
        presenters: conduitos::make::PRESENTER_LINEAR_SERIAL
            | if graphical {
                conduitos::make::PRESENTER_NATIVE_GRAPHICAL
            } else {
                0
            },
        proof_instrumentation: 0,
        face_surface_slots: surface_slots,
        face_surface_bytes: surface_bytes,
    })
}
fn exact(class: &str, expected: &[&str], selected: &[String]) -> Result<(), ConduitosError> {
    if selected.len() != expected.len()
        || expected.iter().any(|value| {
            selected
                .iter()
                .filter(|item| item.as_str() == *value)
                .count()
                != 1
        })
    {
        return Err(refusal(
            "ia32-product-closure-mismatch",
            format!("expected exactly {class}:{expected:?}, found {selected:?}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "ia32_tests.rs"]
mod tests;
