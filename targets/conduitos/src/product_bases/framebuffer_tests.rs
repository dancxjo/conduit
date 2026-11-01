use super::{EffectBaseRefusal, NativeProductBases};
use conduit_core::HostBaseId;
use conduit_observatory::FramebufferBasis;

fn framebuffer() -> FramebufferBasis {
    FramebufferBasis {
        base_id: HostBaseId::from("boot-7/framebuffer-0"),
        width: 800,
        height: 600,
        pitch_bytes: 3200,
        bits_per_pixel: 32,
    }
}

#[test]
fn framebuffer_observation_refuses_a_pitch_shorter_than_one_row() {
    let malformed = FramebufferBasis {
        pitch_bytes: 3199,
        ..framebuffer()
    };
    assert!(matches!(
        NativeProductBases::observe_framebuffer(&malformed),
        Err(EffectBaseRefusal::InvalidProvider)
    ));
}

#[test]
fn framebuffer_observation_refuses_invalid_geometry_and_pixel_width() {
    for (width, height, pitch_bytes, bits_per_pixel) in [
        (0, 600, 3200, 32),
        (800, 0, 3200, 32),
        (u32::MAX, 600, u32::MAX, 32),
        (800, 600, 0, 32),
        (800, 600, 3200, 24),
    ] {
        let malformed = FramebufferBasis {
            width,
            height,
            pitch_bytes,
            bits_per_pixel,
            ..framebuffer()
        };
        assert!(matches!(
            NativeProductBases::observe_framebuffer(&malformed),
            Err(EffectBaseRefusal::InvalidProvider)
        ));
    }
}

#[test]
fn framebuffer_observation_preserves_padded_rows_and_identity_refusal() {
    let padded = FramebufferBasis {
        pitch_bytes: 3264,
        ..framebuffer()
    };
    assert!(NativeProductBases::observe_framebuffer(&padded).is_ok());
    let unnamed = FramebufferBasis {
        base_id: HostBaseId::from(""),
        ..padded
    };
    assert!(matches!(
        NativeProductBases::observe_framebuffer(&unnamed),
        Err(EffectBaseRefusal::InvalidProvider)
    ));
}
