//! CPU contract checks; these do not establish firmware discovery or QEMU pixels.
use super::*;
use crate::{display::DisplayError, make::*, offer::CpuFeatures};
use conduit_face::ManifestationLifecycle;

struct Display {
    writes: u32,
    fail: bool,
}
impl PixelTarget for Display {
    fn format(&self) -> crate::display::DisplayFormat {
        crate::display::DisplayFormat {
            width: 640,
            height: 480,
            pitch: 2560,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
        if self.fail {
            return Err(DisplayError::Lost);
        }
        self.writes += 1;
        Ok(())
    }
}
fn make() -> MakeRecord {
    MakeRecord {
        schema: MAKE_SCHEMA,
        profile_id: "profile:sha256:first-frame",
        build_id: "build:sha256:first-frame",
        image_binding: "image:sha256:first-frame",
        target: "conduitos/aarch64/virt",
        implementations: ALL_KNOWN_IMPLEMENTATIONS & !IMPL_LINEAR_PRESENTER & !IMPL_HTTP_CLIENT,
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
fn offer<'a>(ids: &BootIdentities, make: &'a MakeRecord) -> ImageBoundHostOffer<'a> {
    ImageBoundHostOffer::new(
        ids,
        make,
        CpuFeatures {
            sse2: false,
            rdrand: false,
            invariant_tsc: false,
        },
        16 * 1024 * 1024,
    )
    .unwrap()
}
#[test]
fn canonical_arrival_retains_checked_mask_show_and_exact_source_publication() {
    let ids = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let make = make();
    let mut display = Display {
        writes: 0,
        fail: false,
    };
    let frame =
        NativeFirstFrame::prepare(&ids, &offer(&ids, &make), &make, &mut display, [7; 32]).unwrap();
    let face = frame.face().unwrap();
    let show = frame.show().unwrap();
    let publication = frame.publication().unwrap();
    assert!(face.basis.body_id.is_none());
    assert_eq!(publication.observed_face_basis, face.basis);
    assert_eq!(
        face.basis.source_document_id.as_ref(),
        Some(&publication.producer_plot.source_document_id)
    );
    assert_eq!(
        face.basis.checked_plot_id.as_ref(),
        Some(&publication.producer_plot.checked_plot_id)
    );
    assert_eq!(
        face.basis.plan_id.as_ref(),
        Some(&publication.producer_plan_id)
    );
    assert!(publication.kernel_signs > 0);
    assert_eq!(frame.composition.face_id, face.identity);
    assert_eq!(frame.publication().unwrap().face_id, face.identity.as_str());
    assert_eq!(show.show.face_id, face.identity);
    assert_eq!(show.show.plan_id, frame.composition.plan_id);
    assert_eq!(show.show.active_play_id, frame.composition.active_play_id);
    assert_eq!(show.show.lifecycle, ManifestationLifecycle::Available);
    assert!(display.writes > 0);
    assert!(!frame.mask.route_keyboard().unwrap());
}
#[test]
fn finite_surface_authority_and_failed_scanout_refuse_without_a_frame() {
    let ids = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let make = make();
    let offer = offer(&ids, &make);
    let mut display = Display {
        writes: 0,
        fail: false,
    };
    let mut small = make;
    small.face_surface_bytes = 1;
    assert!(matches!(
        NativeFirstFrame::prepare(&ids, &offer, &small, &mut display, [7; 32]),
        Err("first-frame-display-bound")
    ));
    assert!(NativeFirstFrame::prepare(&ids, &offer, &make, &mut display, [0; 32]).is_err());
    assert_eq!(display.writes, 0);
    display.fail = true;
    assert!(NativeFirstFrame::prepare(&ids, &offer, &make, &mut display, [7; 32]).is_err());
}
