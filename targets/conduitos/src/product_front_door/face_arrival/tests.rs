use super::*;
use alloc::vec;
use conduit_face::{BodyFace, FaceContext, FaceFocus};

pub(super) struct Display;

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
        Ok(())
    }
}

#[test]
fn owner_face_retires_arrival_revision_domain_once() {
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
    let body = conduit_body::Body::born(
        "source/shared-clock".into(),
        "checked/shared-clock".into(),
        1,
        "sign/owner-born".into(),
    )
    .unwrap();
    let face_at = |revision| {
        BodyFace::project(
            &body,
            None,
            revision,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![],
        )
        .unwrap()
        .face
    };
    let local = face_at(9);
    let owner = face_at(7);
    assert_ne!(local.identity, owner.identity);
    assert_eq!(local.revision, 9);
    assert_eq!(owner.revision, 7);
    let mut display = Display;
    arrival.present(local, &mut display).unwrap();
    let local_show = arrival.mask.show().unwrap().show_id.clone();

    arrival.retire_arrival_surface().unwrap();
    assert!(arrival.mask.show().is_none());
    arrival.owner_face = Some(owner.clone());
    let receipt = arrival
        .mask
        .present(
            owner.clone(),
            arrival.next_observation,
            arrival.next_play,
            &mut display,
        )
        .unwrap();
    assert_eq!(receipt.face_id, owner.identity);
    assert_ne!(arrival.mask.show().unwrap().show_id, local_show);
    assert_eq!(arrival.mask.scene().unwrap().face(), &owner);

    arrival.retire_arrival_surface().unwrap();
    assert_eq!(
        arrival
            .mask
            .present(
                owner,
                arrival.next_observation + 1,
                arrival.next_play + 1,
                &mut display
            )
            .map_err(|error| error.as_str()),
        Err("compositor-surface-revision-stale")
    );
}

#[test]
fn route_loss_retires_owner_show_before_read_only_republication() {
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
    let body = conduit_body::Body::born(
        "source/shared-clock".into(),
        "checked/shared-clock".into(),
        1,
        "sign/owner-born".into(),
    )
    .unwrap();
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
    let mut display = Display;
    arrival
        .mask
        .present(face.clone(), 1, 1, &mut display)
        .unwrap();
    arrival.owner_face = Some(face.clone());
    let active_show = arrival.mask.show().unwrap().show_id.clone();

    arrival.retire_owner_surface().unwrap();
    assert!(arrival.mask.show().is_none());
    let read_only = arrival
        .mask
        .present_read_only(face.clone(), 2, 2, &mut display)
        .unwrap();
    assert_eq!(read_only.face_id, face.identity);
    assert_ne!(arrival.mask.show().unwrap().show_id, active_show);
    assert_eq!(arrival.mask.scene().unwrap().face(), &face);
    assert_eq!(
        arrival
            .mask
            .present_read_only(face, 3, 3, &mut display)
            .map_err(|error| error.as_str()),
        Err("compositor-surface-revision-stale")
    );
}

#[test]
fn read_only_owner_standby_requires_fresh_show_for_explicit_activation() {
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
    let body = conduit_body::Body::born(
        "source/shared-clock".into(),
        "checked/shared-clock".into(),
        1,
        "sign/owner-born".into(),
    )
    .unwrap();
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
    let mut display = Display;
    arrival
        .mask
        .present_read_only(face.clone(), 1, 1, &mut display)
        .unwrap();
    arrival.owner_face = Some(face.clone());
    let standby_show = arrival.mask.show().unwrap().show_id.clone();
    assert!(arrival.mask.scene().is_some());

    arrival.retire_owner_surface().unwrap();
    assert!(arrival.mask.show().is_none());
    let activated = arrival
        .mask
        .present(face.clone(), 2, 2, &mut display)
        .unwrap();
    assert_eq!(activated.face_id, face.identity);
    assert_ne!(arrival.mask.show().unwrap().show_id, standby_show);
    assert_eq!(arrival.mask.scene().unwrap().face(), &face);
}
