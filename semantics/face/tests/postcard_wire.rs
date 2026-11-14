#![cfg(all(feature = "postcard-wire", feature = "plot-catalog"))]

#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
mod common;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

use conduit_core::{Quantity, Unit};
use conduit_environment_face::{PreparedObservationFace, SensorStatus};
use conduit_face::{postcard_wire, Face, FaceBasis, ManifestationLifecycle, Show};
use serde::de::DeserializeOwned;

fn environment_face() -> Face {
    let id = "a".repeat(64);
    let body = conduit_body::Body::born(id.clone().into(), id.clone().into(), 1, id.clone().into())
        .unwrap();
    let (_, wake) = body.wake(2, "b".repeat(64).into()).unwrap();
    let basis = FaceBasis {
        body_id: Some(body.body_id),
        wake_id: Some(wake.wake_id),
        source_document_id: Some(id.clone().into()),
        checked_plot_id: Some(id.clone().into()),
        expanded_plot_id: Some(id.clone().into()),
        plan_id: Some(id.clone().into()),
        active_play_id: Some(id.into()),
        sign_ids: vec![],
    };
    let mut prepared =
        PreparedObservationFace::prepare_observed_named(basis, 960, 120000, "Hamm Creek House")
            .unwrap();
    prepared
        .publish(
            Some((
                Quantity::from_decimal(-185, -1, Unit::Celsius).unwrap(),
                Quantity::from_decimal(539, -1, Unit::Percent).unwrap(),
            )),
            SensorStatus::Available,
            u64::MAX,
        )
        .unwrap();
    prepared.current().clone()
}

fn available_show(face: &Face) -> Show {
    let plot = common::checked_renderer_plot();
    let id = "a".repeat(64);
    let plan = common::plan_for(
        &plot,
        common::host(
            &id,
            &id,
            &id,
            &id,
            &id,
            "face/base/wayland-surface@1",
            common::WAYLAND_RESOURCE,
        ),
    );
    let placement = &plan.fragments[0].placements[0];
    let context =
        conduit_face::PreparedRendererContext::prepare(&plan, &placement.placement_id).unwrap();
    let active =
        conduit_core::bind_active_play(&plan.plan_id, &placement.host_id, &placement.boot_id, 1);
    context
        .prepared_show(
            face,
            active,
            "environment".into(),
            "local-segmented-display".into(),
            id.into(),
        )
        .unwrap()
        .transition(ManifestationLifecycle::Available, "s".repeat(64).into())
        .unwrap()
}

// Funded host-side decoding, not an allocation claim about MCU Play. Postcard's
// from_bytes alone permits trailing data, so strict callers must check the tail.
fn decode_exact<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ()> {
    let (value, trailing) = postcard::take_from_bytes(bytes).map_err(|_| ())?;
    if trailing.is_empty() {
        Ok(value)
    } else {
        Err(())
    }
}

#[test]
fn real_quantity_face_and_available_show_round_trip_in_three_kib_without_encoding_allocations() {
    assert_eq!(postcard_wire::ABI, "conduit.face/postcard@1");
    let face = environment_face();
    face.validate().unwrap();
    assert!(serde_json::to_vec(&face).unwrap().len() > 3072);
    let show = available_show(&face);
    let mut face_buffer = [0; 3072];
    let mut show_buffer = [0; 3072];
    let ((face_len, show_len), memory) = allocation_probe::observe(|| {
        (
            postcard_wire::encode_face(&face, &mut face_buffer)
                .unwrap()
                .len(),
            postcard_wire::encode_show(&show, &mut show_buffer)
                .unwrap()
                .len(),
        )
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let decoded: Face = decode_exact(&face_buffer[..face_len]).unwrap();
    assert_eq!(decoded, face);
    decoded.validate().unwrap();
    assert_eq!(
        decode_exact::<Show>(&show_buffer[..show_len]).unwrap(),
        show
    );
    assert_eq!(show.signs.len(), 2);
    for bytes in [&face_buffer[..face_len], &show_buffer[..show_len]] {
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(decode_exact::<Face>(&trailing).is_err());
        assert!(decode_exact::<Show>(&trailing).is_err());
    }
    assert!(decode_exact::<Face>(&face_buffer[..face_len - 1]).is_err());
    assert!(decode_exact::<Show>(&show_buffer[..show_len - 1]).is_err());
    assert!(decode_exact::<Face>(&[255; 8]).is_err());
    assert!(decode_exact::<Show>(&[255; 8]).is_err());
}

#[test]
fn insufficient_buffers_refuse_without_truncating_the_objects() {
    let mut face = environment_face();
    let show = available_show(&face);
    assert!(postcard_wire::encode_face(&face, &mut [0; 1]).is_err());
    assert!(postcard_wire::encode_show(&show, &mut [0; 1]).is_err());
    face.text.push(conduit_face::FaceText {
        subject: "environment".into(),
        text: "x".repeat(1024),
    });
    face.reseal_prepared().unwrap();
    let original = face.clone();
    assert!(postcard_wire::encode_face(&face, &mut [0; 3072]).is_err());
    assert_eq!(face, original);
}
