use super::*;

fn record() -> MakeRecord {
    MakeRecord {
        schema: MAKE_SCHEMA,
        profile_id: "sha256:profile",
        build_id: "build:sha256:build",
        image_binding: "image:sha256:binding",
        target: "conduitos/x86_64/pc",
        implementations: ALL_KNOWN_IMPLEMENTATIONS
            & !IMPL_LINEAR_PRESENTER
            & !IMPL_HTTP_CLIENT
            & !IMPL_STARTUP_CHIME,
        facilities: FACILITY_NATIVE_COMPOSITOR,
        resources: RESOURCE_FACE_SURFACE,
        bases: BASE_DISPLAY_SCANOUT,
        drivers: DRIVER_LINEAR_FRAMEBUFFER,
        presenters: PRESENTER_NATIVE_GRAPHICAL,
        proof_instrumentation: 0,
        face_surface_slots: 4,
        face_surface_bytes: 8 * 1024 * 1024,
        runtime_arena_ceiling: 16 * 1024 * 1024,
        operation_slot_ceiling: 64,
        timer_slot_ceiling: 32,
        evidence_item_ceiling: 1024,
    }
}

#[test]
fn exact_record_is_bounded_and_inventory_specific() {
    let record = record();
    assert_eq!(record.validate(4 * 1024 * 1024), Ok(()));
    assert!(record.includes(IMPL_KEYBOARD));
    assert!(
        !MakeRecord {
            implementations: IMPL_TIME_TICK,
            ..record
        }
        .includes(IMPL_KEYBOARD)
    );
}

#[test]
fn startup_chime_requires_the_selected_audio_base_and_driver() {
    let record = MakeRecord {
        target: "conduitos/armv6/raspberry-pi-model-b-plus-v1.2",
        implementations: IMPL_TIME_TICK
            | IMPL_NATIVE_PRESENTER
            | IMPL_LINEAR_PRESENTER
            | IMPL_STARTUP_CHIME,
        bases: BASE_DISPLAY_SCANOUT | BASE_SERIAL_TEXT | BASE_PCM_AUDIO,
        drivers: DRIVER_LINEAR_FRAMEBUFFER | DRIVER_PL011_SERIAL | DRIVER_PWM_AUDIO,
        presenters: PRESENTER_NATIVE_GRAPHICAL | PRESENTER_LINEAR_SERIAL,
        ..record()
    };
    assert_eq!(record.validate(4 * 1024 * 1024), Ok(()));
    for changed in [
        MakeRecord {
            bases: record.bases & !BASE_PCM_AUDIO,
            ..record
        },
        MakeRecord {
            drivers: record.drivers & !DRIVER_PWM_AUDIO,
            ..record
        },
        MakeRecord {
            implementations: record.implementations & !IMPL_STARTUP_CHIME,
            ..record
        },
    ] {
        assert_eq!(
            changed.validate(4 * 1024 * 1024),
            Err(MakeError::UnknownInventory)
        );
    }
}

#[test]
fn bplus_usb_inventory_requires_its_actual_base_and_driver_pair() {
    let mut value = record();
    value.target = "conduitos/armv6/raspberry-pi-model-b-plus-v1.2";
    value.implementations = IMPL_TIME_TICK | IMPL_LINEAR_PRESENTER | IMPL_USB_TRANSPORT;
    value.facilities = 0;
    value.resources = 0;
    value.bases = BASE_SERIAL_TEXT | BASE_USB_CONTROLLER;
    value.drivers = DRIVER_PL011_SERIAL | DRIVER_DWC2_USB;
    value.presenters = PRESENTER_LINEAR_SERIAL;
    value.face_surface_slots = 0;
    value.face_surface_bytes = 0;
    assert_eq!(value.validate(4 * 1024 * 1024), Ok(()));
    for changed in [
        MakeRecord {
            bases: value.bases & !BASE_USB_CONTROLLER,
            ..value
        },
        MakeRecord {
            drivers: value.drivers & !DRIVER_DWC2_USB,
            ..value
        },
        MakeRecord {
            implementations: value.implementations & !IMPL_USB_TRANSPORT,
            ..value
        },
    ] {
        assert_eq!(
            changed.validate(4 * 1024 * 1024),
            Err(MakeError::UnknownInventory)
        );
    }
}

#[test]
fn schema_identity_inventory_and_runtime_ceiling_fail_separately() {
    let record = record();
    assert_eq!(
        MakeRecord {
            schema: "future",
            ..record
        }
        .validate(1),
        Err(MakeError::UnsupportedSchema)
    );
    assert_eq!(
        MakeRecord {
            profile_id: "bad value",
            ..record
        }
        .validate(1),
        Err(MakeError::MalformedIdentity)
    );
    assert_eq!(
        MakeRecord {
            implementations: 0,
            ..record
        }
        .validate(1),
        Err(MakeError::UnknownInventory)
    );
    assert_eq!(
        record.validate(record.runtime_arena_ceiling + 1),
        Err(MakeError::RuntimeArenaExceeded)
    );
}

#[test]
fn linked_inventory_rejects_claimed_or_compiled_graphics_mismatches() {
    let record = record();
    for malformed in [
        MakeRecord {
            presenters: 0,
            ..record
        },
        MakeRecord {
            drivers: 0,
            ..record
        },
        MakeRecord { bases: 0, ..record },
        MakeRecord {
            resources: 0,
            ..record
        },
        MakeRecord {
            facilities: 0,
            ..record
        },
        MakeRecord {
            implementations: record.implementations & !IMPL_NATIVE_PRESENTER,
            ..record
        },
        MakeRecord {
            face_surface_slots: 0,
            ..record
        },
        MakeRecord {
            face_surface_bytes: 0,
            ..record
        },
        MakeRecord {
            proof_instrumentation: 1 << 15,
            ..record
        },
    ] {
        assert_eq!(malformed.validate(1), Err(MakeError::UnknownInventory));
    }
}

#[test]
fn usb_transport_inventory_rejects_unsupported_targets_and_unknown_bits() {
    let native = record();
    assert!(native.includes(IMPL_USB_TRANSPORT));
    assert_eq!(native.validate(1), Ok(()));
    for target in [
        "conduitos/ia32/pc",
        "conduitos/riscv64/virt",
        "conduitos/loongarch64/virt",
        "conduitos/aarch64/orange-pi-5-rk3588s",
    ] {
        assert_eq!(
            MakeRecord { target, ..native }.validate(1),
            Err(MakeError::UnknownInventory)
        );
    }
    assert_eq!(
        MakeRecord {
            implementations: native.implementations | (1 << 15),
            ..native
        }
        .validate(1),
        Err(MakeError::UnknownInventory)
    );
}

#[test]
fn aarch64_graphical_requires_one_complete_native_closure() {
    let graphical = MakeRecord {
        target: "conduitos/aarch64/virt",
        implementations: IMPL_TIME_TICK
            | IMPL_TICK_FACE
            | IMPL_TEXT_LITERAL
            | IMPL_TEXT_UPPER
            | IMPL_TEXT_FACE
            | IMPL_NATIVE_PRESENTER
            | IMPL_LINEAR_PRESENTER
            | IMPL_USB_TRANSPORT,
        bases: BASE_DISPLAY_SCANOUT | BASE_SERIAL_TEXT,
        drivers: DRIVER_LINEAR_FRAMEBUFFER | DRIVER_PL011_SERIAL,
        presenters: PRESENTER_NATIVE_GRAPHICAL | PRESENTER_LINEAR_SERIAL,
        ..record()
    };
    assert_eq!(graphical.validate(1), Ok(()));
    for malformed in [
        MakeRecord {
            implementations: graphical.implementations & !IMPL_LINEAR_PRESENTER,
            ..graphical
        },
        MakeRecord {
            drivers: DRIVER_PL011_SERIAL,
            ..graphical
        },
        MakeRecord {
            face_surface_bytes: 0,
            ..graphical
        },
    ] {
        assert_eq!(malformed.validate(1), Err(MakeError::UnknownInventory));
    }
}

#[test]
fn aarch64_linear_closure_remains_distinct_from_graphical() {
    let x86 = record();
    let record = MakeRecord {
        target: "conduitos/aarch64/virt",
        implementations: (x86.implementations
            & !(IMPL_NATIVE_PRESENTER
                | IMPL_KEYBOARD
                | IMPL_PC_SPEAKER
                | IMPL_OPL2
                | IMPL_USB_TRANSPORT))
            | IMPL_LINEAR_PRESENTER,
        facilities: 0,
        resources: 0,
        bases: BASE_SERIAL_TEXT,
        drivers: DRIVER_PL011_SERIAL,
        presenters: PRESENTER_LINEAR_SERIAL,
        face_surface_slots: 0,
        face_surface_bytes: 0,
        ..x86
    };
    assert_eq!(record.validate(1), Ok(()));
    assert_eq!(
        MakeRecord {
            implementations: record.implementations | IMPL_USB_TRANSPORT,
            ..record
        }
        .validate(1),
        Ok(())
    );
    for malformed in [
        MakeRecord {
            target: "conduitos/x86_64/pc",
            ..record
        },
        MakeRecord {
            bases: BASE_DISPLAY_SCANOUT,
            ..record
        },
        MakeRecord {
            drivers: DRIVER_LINEAR_FRAMEBUFFER,
            ..record
        },
        MakeRecord {
            presenters: PRESENTER_NATIVE_GRAPHICAL,
            ..record
        },
    ] {
        assert_eq!(malformed.validate(1), Err(MakeError::UnknownInventory));
    }
}

#[test]
fn three_b_plus_product_inventory_keeps_exact_target_and_usb_pairing() {
    let value = MakeRecord {
        target: "conduitos/aarch64/raspberry-pi-3-model-b-plus-rev-1.3",
        implementations: IMPL_TIME_TICK
            | IMPL_NATIVE_PRESENTER
            | IMPL_LINEAR_PRESENTER
            | IMPL_USB_TRANSPORT
            | IMPL_STARTUP_CHIME,
        bases: BASE_DISPLAY_SCANOUT | BASE_SERIAL_TEXT | BASE_PCM_AUDIO | BASE_USB_CONTROLLER,
        drivers: DRIVER_LINEAR_FRAMEBUFFER
            | DRIVER_PL011_SERIAL
            | DRIVER_PWM_AUDIO
            | DRIVER_DWC2_USB,
        presenters: PRESENTER_NATIVE_GRAPHICAL | PRESENTER_LINEAR_SERIAL,
        ..record()
    };
    assert_eq!(value.validate(value.runtime_arena_ceiling), Ok(()));
    assert_eq!(
        MakeRecord {
            target: "conduitos/aarch64/raspberry-pi-3-model-b",
            ..value
        }
        .validate(1),
        Err(MakeError::WrongTarget)
    );
    for changed in [
        MakeRecord {
            bases: value.bases & !BASE_USB_CONTROLLER,
            ..value
        },
        MakeRecord {
            drivers: value.drivers & !DRIVER_DWC2_USB,
            ..value
        },
        MakeRecord {
            implementations: value.implementations & !IMPL_USB_TRANSPORT,
            ..value
        },
    ] {
        assert_eq!(changed.validate(1), Err(MakeError::UnknownInventory));
    }
}

#[cfg(feature = "aarch64-rpi-3b-plus")]
#[test]
fn embedded_three_b_plus_make_record_passes_runtime_admission() {
    assert_eq!(
        EMBEDDED_MAKE.target,
        "conduitos/aarch64/raspberry-pi-3-model-b-plus-rev-1.3"
    );
    assert_eq!(
        EMBEDDED_MAKE.validate(EMBEDDED_MAKE.runtime_arena_ceiling),
        Ok(())
    );
}
