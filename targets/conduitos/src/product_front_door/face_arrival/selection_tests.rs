//! CPU preparation refusal before the local Mask swap, not owner-route evidence.
use super::*;
use alloc::{boxed::Box, vec};
use conduit_body::{
    Body, MembershipCredential, PortableAdmissionReceipt, SPAWN_ADMISSION_RECEIPT_SCHEMA,
};
use conduit_core::{HostAdvertisement, PROTOCOL_VERSION};
use conduit_face::{
    BodyFace, FaceContext, FaceFocus, OWNER_FACE_RESPONSE_SCHEMA, OwnerFaceSnapshotResponse,
};

struct Display(usize);
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
    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), crate::display::DisplayError> {
        self.0 += 1;
        Ok(())
    }
}

fn snapshot() -> crate::native_guest_face::NativeGuestFace {
    let body = Body::born(
        "source/replacement".into(),
        "checked/replacement".into(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/native".into(),
        boot_id: "boot/native".into(),
        offer_generation: OfferGeneration(1),
        profile: "profile/native".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let credential = MembershipCredential {
        credential_id: serde_json::from_str("\"credential/replacement\"").unwrap(),
        body_id: body.body_id.clone(),
        part_id: serde_json::from_str("\"part/replacement\"").unwrap(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        issued_at_millis: 1,
    };
    let receipt = PortableAdmissionReceipt {
        schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential,
        host_advertisement: host,
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
    };
    let face = BodyFace::project(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .face;
    crate::native_guest_face::NativeGuestFace::from_owner_response(
        &receipt,
        OwnerFaceSnapshotResponse::Snapshot {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            face: Box::new(face),
            interactions_admitted: false,
            route: None,
        },
    )
    .unwrap()
}

#[test]
fn failed_pre_swap_preparation_preserves_show_preferences_and_publication() {
    let provider = crate::product_bases::fixture_surface_provider();
    let mut arrival = FaceArrival::prepare(
        "host/native".into(),
        "boot/native".into(),
        OfferGeneration(1),
        "build",
        provider.entry.base_id.clone(),
        &provider,
        None,
    )
    .unwrap();
    let fresh = snapshot();
    let mut display = Display(0);
    // Install the ordinary read-only CPU fixture without guest UART evidence.
    // replace_owner_mask below still executes its actual production boundary.
    arrival.retire_arrival_surface().unwrap();
    arrival.owner_face = Some(fresh.face().clone());
    arrival
        .mask
        .present_read_only(
            fresh.face().clone(),
            arrival.next_observation,
            arrival.next_play,
            &mut display,
        )
        .unwrap();
    let show = arrival.mask.show().unwrap().clone();
    let expression = arrival.mask.visual_expression().cloned();
    let sequences = arrival.next_publication_sequences();
    let painted = display.0;
    for scale in [99, 201] {
        assert_eq!(
            arrival.replace_owner_mask(&fresh, (true, scale), &mut display),
            Err("native-face-mask-face-refused")
        );
        assert_eq!(arrival.mask.show(), Some(&show));
        assert_eq!(arrival.mask.visual_expression(), expression.as_ref());
        assert!(!arrival.preparation.forced_colors);
        assert_eq!(arrival.preparation.minimum_text_scale_percent, 100);
        assert_eq!(arrival.next_publication_sequences(), sequences);
        assert_eq!(display.0, painted);
        assert_eq!(arrival.owner_face.as_ref(), Some(fresh.face()));
    }
}
