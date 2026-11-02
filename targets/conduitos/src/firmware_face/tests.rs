//! Owning CPU contracts recovered from the older NativeFirstFrame preparation.
//! These checks do not establish firmware mapping, QEMU pixels or physical input.
use super::*;
use crate::{display::DisplayError, make::*, offer::CpuFeatures, offer_make::ImageBoundHostOffer};
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU32, Ordering};

struct Display {
    attempts: Arc<AtomicU32>,
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
        self.attempts.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(DisplayError::Lost)
        } else {
            Ok(())
        }
    }
}
fn make() -> MakeRecord {
    MakeRecord {
        schema: MAKE_SCHEMA,
        profile_id: "profile:sha256:first-frame",
        build_id: "build:sha256:first-frame",
        image_binding: "image:sha256:first-frame",
        target: "conduitos/aarch64/virt",
        implementations: IMPL_TIME_TICK
            | IMPL_TICK_FACE
            | IMPL_TEXT_LITERAL
            | IMPL_TEXT_UPPER
            | IMPL_TEXT_FACE
            | IMPL_NATIVE_PRESENTER
            | IMPL_LINEAR_PRESENTER,
        facilities: FACILITY_NATIVE_COMPOSITOR,
        resources: RESOURCE_FACE_SURFACE,
        bases: BASE_DISPLAY_SCANOUT | BASE_SERIAL_TEXT,
        drivers: DRIVER_LINEAR_FRAMEBUFFER | DRIVER_PL011_SERIAL,
        presenters: PRESENTER_NATIVE_GRAPHICAL | PRESENTER_LINEAR_SERIAL,
        proof_instrumentation: 0,
        face_surface_slots: 4,
        face_surface_bytes: 8 * 1024 * 1024,
        runtime_arena_ceiling: NATIVE_PREPARATION_ARENA_BYTES,
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
        NATIVE_PREPARATION_ARENA_BYTES,
    )
    .unwrap()
}
fn identities() -> BootIdentities {
    BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    }
}

#[test]
fn native_face_retains_source_publication_show_and_readonly_scanout() {
    let ids = identities();
    let make = make();
    let attempts = Arc::new(AtomicU32::new(0));
    let frame = FirmwareFace::prepare(
        Display {
            attempts: attempts.clone(),
            fail: false,
        },
        &ids,
        &offer(&ids, &make),
        &make,
        [7; 32],
    )
    .unwrap();
    let mask = &frame._mask;
    let face = mask.scene().unwrap().face();
    let show = mask.show().unwrap();
    let publication = mask.publication().unwrap();
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
    assert_eq!(publication.face_id, face.identity.as_str());
    assert_eq!(publication.face_revision, face.revision);
    assert_eq!(show.face_id, face.identity);
    assert_eq!(show.face_revision, face.revision);
    assert_eq!(
        show.show.lifecycle,
        conduit_face::ManifestationLifecycle::Available
    );
    let rendered = mask.render_receipt().unwrap();
    assert_eq!(rendered.face_id, face.identity.as_str());
    assert_eq!(rendered.face_revision, face.revision);
    assert_eq!(show.show.plan_id, rendered.mask_plan_id);
    assert_eq!(show.show.active_play_id, rendered.active_play_id);
    assert_eq!(rendered.display_base_id, frame.framebuffer_basis().base_id);
    assert!(rendered.scanout_pixels_written > 0);
    assert!(attempts.load(Ordering::SeqCst) > 0);
    let prepared = mask.scene().unwrap().frame().unwrap();
    assert!(prepared.hits.is_empty());
    assert!(matches!(
        mask.scene().unwrap().interaction(
            &face.identity,
            face.revision,
            crate::native_face_scene::FaceControl {
                action: 0,
                argument: None
            },
            show,
            alloc::vec![],
            1,
        ),
        Err(crate::native_face_scene::FaceSceneError::InputUnavailable)
    ));
    assert!(
        prepared
            .scene
            .commands()
            .iter()
            .any(|c| c.payload() == "Read only · No local input")
    );
    assert!(
        !prepared
            .scene
            .commands()
            .iter()
            .any(|c| c.payload().contains("PgUp") || c.payload().contains("F2"))
    );
    let evidence: serde_json::Value = serde_json::from_slice(frame.evidence()).unwrap();
    assert!(frame.evidence().len() <= MAX_EVIDENCE_BYTES);
    let encoded_configuration = &evidence["component_configuration"];
    let mut independent = encoded_configuration.clone();
    independent
        .as_object_mut()
        .unwrap()
        .remove("evidence_encoding");
    for field in ["recipe", "look", "layout"] {
        let Some(value) = independent.get_mut(field) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let encoded = &value["canonical_value"];
        assert_eq!(encoded["encoding"], "hex@1");
        let text = encoded["value"].as_str().unwrap();
        assert_eq!(text.len() % 2, 0);
        let bytes: alloc::vec::Vec<u8> = (0..text.len())
            .step_by(2)
            .map(|offset| u8::from_str_radix(&text[offset..offset + 2], 16).unwrap())
            .collect();
        assert_eq!(
            encoded["byte_length"].as_u64().unwrap() as usize,
            bytes.len()
        );
        value["canonical_value"] = serde_json::to_value(bytes).unwrap();
    }
    let original_configuration =
        serde_json::to_value(mask.component_configuration().unwrap()).unwrap();
    assert_eq!(independent, original_configuration);
    assert_eq!(
        crate::firmware_face_evidence::decode_configuration(encoded_configuration).unwrap(),
        original_configuration
    );
    assert_eq!(evidence["face"]["identity"], face.identity.as_str());
    assert_eq!(evidence["show"]["face_id"], face.identity.as_str());
    assert_eq!(evidence["local_input_available"], false);
    assert_eq!(evidence["interactions_admitted"], false);
    assert_eq!(evidence["physical_scanout_proved"], false);
}

#[test]
fn native_face_refuses_finite_surfaces_invalid_issuer_and_failed_scanout() {
    let ids = identities();
    let make = make();
    let offer = offer(&ids, &make);
    let attempts = Arc::new(AtomicU32::new(0));
    let display = |fail| Display {
        attempts: attempts.clone(),
        fail,
    };
    // One scanout fits, but the admitted offscreen plus scanout pair does not.
    let mut small = make;
    small.face_surface_bytes = 640 * 480 * 4;
    assert!(matches!(
        FirmwareFace::prepare(display(false), &ids, &offer, &small, [7; 32]),
        Err("firmware-face-surface-budget-unavailable")
    ));
    assert!(FirmwareFace::prepare(display(false), &ids, &offer, &make, [0; 32]).is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
    assert!(FirmwareFace::prepare(display(true), &ids, &offer, &make, [7; 32]).is_err());
    assert!(
        attempts.load(Ordering::SeqCst) > 0,
        "the valid path reaches the failing display"
    );
}
