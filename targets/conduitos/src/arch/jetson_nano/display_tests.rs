use super::*;

fn firmware() -> Region {
    Region::new(0x9200_0000, 8 * 1024 * 1024).unwrap()
}

fn valid() -> Snapshot {
    Snapshot {
        state_access: 1,
        selected_window: 0x10,
        display_command: 1 << 5,
        smmu_config: 0,
        active_size: 1080 << 16 | 1920,
        interlace_control: 0,
        options: 1 << 30,
        byte_swap: 0,
        buffer_control: 0,
        color_depth: 12,
        position: 0,
        size: 1080 << 16 | 1920,
        prescaled_size: (1080 << 16) | (1920 * 4),
        initial_dda_h: 0,
        initial_dda_v: 0,
        dda_inc: 0x1000_1000,
        stride: 7680,
        buffer_stride: 0,
        uv_stride: 0,
        address_mode: 0,
        address: 0x9200_0000,
        address_high: 0,
        horizontal_offset: 0,
        vertical_offset: 0,
        surface_kind: 0,
    }
}

#[test]
fn retained_linear_window_preserves_exact_firmware_extent() {
    let descriptor = valid().descriptor(firmware(), &Regions::new()).unwrap();
    assert_eq!(descriptor.memory, firmware());
    assert_eq!(
        (descriptor.width, descriptor.height, descriptor.stride),
        (1920, 1080, 7680)
    );
    assert_eq!(descriptor.format, PixelFormat::Bgra8888);
    let mut s = valid();
    s.color_depth = 13;
    assert_eq!(
        s.descriptor(firmware(), &Regions::new()).unwrap().format,
        PixelFormat::Rgba8888
    );
}

#[test]
fn refuses_shadow_stopped_or_translated_scanout() {
    for (snapshot, refusal) in [
        (
            Snapshot {
                state_access: 0,
                ..valid()
            },
            Refusal::InactiveBank,
        ),
        (
            Snapshot {
                selected_window: 0x30,
                ..valid()
            },
            Refusal::WindowSelection,
        ),
        (
            Snapshot {
                display_command: 0,
                ..valid()
            },
            Refusal::NotContinuous,
        ),
        (
            Snapshot {
                smmu_config: 1,
                ..valid()
            },
            Refusal::Translation,
        ),
    ] {
        assert_eq!(
            snapshot.descriptor(firmware(), &Regions::new()),
            Err(refusal)
        );
    }
}

#[test]
fn refuses_tiling_scaling_cropping_and_unknown_color() {
    for s in [
        Snapshot {
            surface_kind: 2,
            ..valid()
        },
        Snapshot {
            address_mode: 1,
            ..valid()
        },
        Snapshot {
            horizontal_offset: 4,
            ..valid()
        },
        Snapshot {
            vertical_offset: 1,
            ..valid()
        },
        Snapshot {
            options: (1 << 30) | 1,
            ..valid()
        },
        Snapshot {
            dda_inc: 0x2000_1000,
            ..valid()
        },
        Snapshot {
            byte_swap: 1,
            ..valid()
        },
    ] {
        assert_eq!(
            s.descriptor(firmware(), &Regions::new()),
            Err(Refusal::Layout)
        );
    }
    assert_eq!(
        Snapshot {
            color_depth: 35,
            ..valid()
        }
        .descriptor(firmware(), &Regions::new()),
        Err(Refusal::Format)
    );
}

#[test]
fn refuses_guessed_geometry_or_address() {
    for s in [
        Snapshot { size: 0, ..valid() },
        Snapshot {
            stride: 7676,
            ..valid()
        },
        Snapshot {
            stride: u32::MAX,
            ..valid()
        },
        Snapshot {
            prescaled_size: (1080 << 16) | (1280 * 4),
            ..valid()
        },
    ] {
        assert_eq!(
            s.descriptor(firmware(), &Regions::new()),
            Err(Refusal::Geometry)
        );
    }
    for s in [
        Snapshot {
            address: 0x9300_0000,
            ..valid()
        },
        Snapshot {
            address_high: 1,
            ..valid()
        },
    ] {
        assert_eq!(
            s.descriptor(firmware(), &Regions::new()),
            Err(Refusal::Address)
        );
    }
    assert_eq!(
        valid().descriptor(Region::new(0x9200_0000, 4096).unwrap(), &Regions::new()),
        Err(Refusal::Address)
    );
}

#[test]
fn framebuffer_cannot_alias_image_or_other_live_reservations() {
    let mut protected = Regions::new();
    protected
        .push(Region::new(0x9200_1000, 4096).unwrap(), true)
        .unwrap();
    assert_eq!(
        valid().descriptor(firmware(), &protected),
        Err(Refusal::Reserved)
    );
}

#[test]
fn capture_is_finite_and_requires_stable_registers() {
    let mut reads = 0;
    let snapshot = Snapshot::capture(
        |_| {
            reads += 1;
            0
        },
        |_| 0,
    )
    .unwrap();
    assert_eq!(snapshot.state_access, 0);
    assert_eq!(reads, 48);
    let mut accesses = 0;
    assert_eq!(
        Snapshot::capture(
            |reg| {
                if reg == 0x040 {
                    accesses += 1;
                    accesses
                } else {
                    0
                }
            },
            |_| 0
        ),
        Err(Refusal::Unstable)
    );
}
