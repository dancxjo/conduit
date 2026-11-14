//! Exact ARMv6 B+ serial plus optional firmware-framebuffer product closure.
use super::{
    refusal, BuildManifest, ConduitosError, TargetBuildInputs, FRAMEBUFFER_DRIVER,
    LINEAR_SERIAL_PRESENTER, NATIVE_COMPOSITOR, NATIVE_PRESENTER, SCANOUT_BASE, SERIAL_TEXT_BASE,
};

const PL011_DRIVER: &str = "raspberry-pi/pl011@1";

pub(crate) fn lower_armv6_bplus(
    manifest: &BuildManifest,
) -> Result<TargetBuildInputs, ConduitosError> {
    let graphical = manifest.presenters.iter().any(|p| p == NATIVE_PRESENTER);
    let mut presenters = vec![LINEAR_SERIAL_PRESENTER];
    let mut bases = vec![SERIAL_TEXT_BASE, "audio/pcm-output", "usb/host-controller"];
    let mut drivers = vec![
        PL011_DRIVER,
        "raspberry-pi/pwm-jack@1",
        "raspberry-pi/dwc2@1",
    ];
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
    if !manifest.profile_fragments.is_empty() {
        return Err(refusal(
            "armv6-product-closure-mismatch",
            "ordinary ARMv6 B+ product cannot select proof instrumentation".into(),
        ));
    }
    for base in &manifest.base_selections {
        let expected = if base.kind == SERIAL_TEXT_BASE {
            PL011_DRIVER
        } else if base.kind == "usb/host-controller" {
            "raspberry-pi/dwc2@1"
        } else if base.kind == "audio/pcm-output" {
            "raspberry-pi/pwm-jack@1"
        } else {
            FRAMEBUFFER_DRIVER
        };
        if base.driver != expected {
            return Err(refusal(
                if base.kind == SERIAL_TEXT_BASE {
                    "armv6-serial-driver-mismatch"
                } else {
                    "armv6-scanout-driver-mismatch"
                },
                format!(
                    "base {} selects {}, expected {expected}",
                    base.kind, base.driver
                ),
            ));
        }
    }
    let mut graphical_manifest = manifest.clone();
    for call in [
        "conduit.host/usb-control@1",
        "conduit.host/usb-endpoint-read@2",
        "conduit.host/monotonic-clock-at@1",
    ] {
        if graphical_manifest
            .host_calls
            .iter()
            .filter(|c| c.as_str() == call)
            .count()
            != 1
        {
            return Err(refusal(
                "armv6-usb-host-call-missing",
                format!("missing exact native operation {call}"),
            ));
        }
        graphical_manifest.host_calls.retain(|c| c != call);
    }

    if graphical_manifest
        .host_calls
        .iter()
        .filter(|c| c.as_str() == "conduit.host/native-startup-chime@1")
        .count()
        != 1
    {
        return Err(refusal(
            "armv6-audio-host-call-missing",
            "exact optional audio operation required".into(),
        ));
    }
    graphical_manifest
        .host_calls
        .retain(|c| c != "conduit.host/native-startup-chime@1");
    let (surface_slots, surface_bytes) = super::graphical::lower(
        &graphical_manifest,
        graphical,
        "armv6-product-closure-mismatch",
    )?;
    let mut features = vec!["armv6-product", "native-startup-chime"];
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
            | conduitos::make::IMPL_STARTUP_CHIME
            | conduitos::make::IMPL_USB_TRANSPORT
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
            | conduitos::make::BASE_PCM_AUDIO
            | conduitos::make::BASE_USB_CONTROLLER
            | if graphical {
                conduitos::make::BASE_DISPLAY_SCANOUT
            } else {
                0
            },
        drivers: conduitos::make::DRIVER_PL011_SERIAL
            | conduitos::make::DRIVER_PWM_AUDIO
            | conduitos::make::DRIVER_DWC2_USB
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
            "armv6-product-closure-mismatch",
            format!("expected exactly {class}:{expected:?}, found {selected:?}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_make::{
        build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
        BuildInputs,
    };
    fn manifest() -> BuildManifest {
        let profile = check_host_configuration(
            parse_host_configuration_conduit(include_str!(
                "../../../profiles/conduitos-armv6-bplus.host.conduit"
            ))
            .unwrap(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
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
    fn checked_bplus_profile_keeps_shared_serial_graphics_and_native_chime() {
        let manifest = manifest();
        let lowered = lower_armv6_bplus(&manifest).unwrap();
        assert!(lowered.cargo_features.contains(&"armv6-product"));
        assert!(lowered.cargo_features.contains(&"native-startup-chime"));
        assert!(manifest
            .presenters
            .iter()
            .any(|p| p == LINEAR_SERIAL_PRESENTER));
        assert!(manifest.presenters.iter().any(|p| p == NATIVE_PRESENTER));
        assert_eq!(lowered.proof_instrumentation, 0);
        assert!(lowered.implementations & conduitos::make::IMPL_USB_TRANSPORT != 0);
        assert!(lowered.bases & conduitos::make::BASE_USB_CONTROLLER != 0);
        assert!(lowered.drivers & conduitos::make::DRIVER_DWC2_USB != 0);
    }
    #[test]
    fn missing_or_foreign_devices_and_audio_calls_refuse() {
        let original = manifest();
        let mut changed = original.clone();
        changed.base_selections[0].driver = "conduitos/pl011@1".into();
        assert!(lower_armv6_bplus(&changed).is_err());
        let mut changed = original.clone();
        changed
            .host_calls
            .retain(|c| c != "conduit.host/native-startup-chime@1");
        assert!(lower_armv6_bplus(&changed).is_err());
        let mut changed = original;
        changed
            .base_selections
            .retain(|b| b.kind != "audio/pcm-output");
        assert!(lower_armv6_bplus(&changed).is_err());
    }
}
