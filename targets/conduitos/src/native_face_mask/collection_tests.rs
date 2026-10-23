//! Profile admission and retirement, not platform/QEMU proof.
use super::super::*;
use crate::native_face_snapshot::{COLLECTION_SNAPSHOT_BYTES, MAX_SNAPSHOT_BYTES};
use conduit_face::{FaceRole, FaceSubject, FaceText};

fn collection() -> NativeFaceMask {
    let provider = crate::product_bases::fixture_surface_provider();
    NativeFaceMask::prepare_collection(
        "host/native".into(),
        "boot/native".into(),
        OfferGeneration(1),
        "build",
        provider.entry.base_id.clone(),
        "surface/collection",
        &provider,
    )
    .unwrap()
}
fn face(mask: &NativeFaceMask, count: usize) -> Face {
    let basis = tests::draft()
        .face(&mask.birth_basis("collection/profile"))
        .unwrap()
        .basis;
    Face::new_with_semantics(
        1,
        basis,
        alloc::vec![FaceSubject {
            identity: "host/collection-observation".into(),
            role: FaceRole::Host,
            name: "Bounded observation".into(),
        }],
        alloc::vec![],
        alloc::vec![],
        (0..count)
            .map(|_| FaceText {
                subject: "host/collection-observation".into(),
                text: "x".repeat(1024),
            })
            .collect(),
        alloc::vec![],
        alloc::vec![],
    )
    .unwrap()
}
#[test]
fn explicit_collection_mask_admits_large_forwarding_and_reports_matching_storage() {
    let mut mask = collection();
    let face = face(&mask, 64);
    let bytes = serde_json::to_vec(&face).unwrap().len();
    assert!((MAX_SNAPSHOT_BYTES + 1..COLLECTION_SNAPSHOT_BYTES).contains(&bytes));
    let profile = conduit_conduitos_mask_offer::NativeMaskStorageProfile::Collection;
    assert!(
        mask.mask
            .planned_mask
            .plan
            .fragments
            .iter()
            .flat_map(|f| &f.fore_ports)
            .any(|port| port.front_port_id.as_str() == "face"
                && port.byte_capacity == profile.bytes())
    );
    let mut display = tests::CountingDisplay::default();
    mask.present(face.clone(), 1, 1, &mut display).unwrap();
    let receipt = mask.publication().unwrap();
    assert_eq!(receipt.face_id, face.identity.as_str());
    assert_eq!(receipt.encoded_bytes as usize, bytes);
    assert_eq!(
        receipt
            .collection_storage
            .as_ref()
            .unwrap()
            .maximum_value_bytes,
        profile.bytes()
    );
    let storage = receipt.collection_storage.as_ref().unwrap();
    assert!(storage.value_slots > 0);
    assert_eq!(
        storage.reserved_value_payload_bytes,
        storage.value_slots * profile.bytes() as usize
    );
    assert!(storage.reserved_value_storage_bytes >= storage.reserved_value_payload_bytes);
    assert!(storage.encoded_input_capacity >= bytes);
    assert!(storage.observed_output_capacity >= bytes);
    assert_eq!(
        mask.render_receipt()
            .unwrap()
            .collection_storage
            .as_ref()
            .unwrap()
            .maximum_value_bytes,
        profile.bytes()
    );
    assert_eq!(mask.show().unwrap().face_id, face.identity);
    assert!(
        mask.producer
            .plan()
            .fragments
            .iter()
            .flat_map(|f| &f.fore_ports)
            .all(|port| port.byte_capacity == profile.bytes())
    );
}
#[test]
fn standard_stays_small_and_collection_overflow_retires_prior_acknowledgement() {
    let (mut standard, _) = tests::fixture();
    let large = face(&standard, 64);
    let mut display = tests::CountingDisplay::default();
    assert_eq!(
        standard.present(large, 1, 1, &mut display),
        Err(NativeFaceMaskError::Producer(FaceSnapshotRefusal::Capacity))
    );
    assert!(standard.show().is_none());
    let mut mask = collection();
    mask.present(face(&mask, 64), 1, 1, &mut display).unwrap();
    let beyond = face(&mask, 300);
    assert!(serde_json::to_vec(&beyond).unwrap().len() > COLLECTION_SNAPSHOT_BYTES);
    assert_eq!(
        mask.present(beyond, 2, 2, &mut display),
        Err(NativeFaceMaskError::Producer(FaceSnapshotRefusal::Capacity))
    );
    assert!(mask.show().is_none());
    assert!(mask.publication().is_none());
    assert!(mask.render_receipt().is_none());
    assert!(mask.route_keyboard().is_err());
}
#[test]
fn collection_preparation_requires_exact_observed_display_base() {
    let provider = crate::product_bases::fixture_surface_provider();
    assert!(matches!(
        NativeFaceMask::prepare_collection(
            "host/native".into(),
            "boot/native".into(),
            OfferGeneration(1),
            "build",
            "base/foreign".into(),
            "surface/collection",
            &provider
        ),
        Err(NativeFaceMaskError::Plan)
    ));
    let mut unavailable = provider.clone();
    unavailable.entry.resources.clear();
    assert!(
        NativeFaceMask::prepare_collection(
            "host/native".into(),
            "boot/native".into(),
            OfferGeneration(1),
            "build",
            unavailable.entry.base_id.clone(),
            "surface/collection",
            &unavailable
        )
        .is_err()
    );
    assert!(
        NativeFaceMask::prepare_collection(
            "host/native".into(),
            "boot/native".into(),
            OfferGeneration(0),
            "build",
            provider.entry.base_id.clone(),
            "surface/collection",
            &provider
        )
        .is_err()
    );
}
#[test]
fn larger_producer_cannot_make_a_standard_mask_stage_accept_its_payload() {
    let mut mask = collection();
    let (standard, _) = tests::fixture();
    mask.mask = standard.mask;
    let mut display = tests::CountingDisplay::default();
    assert!(mask.present(face(&mask, 64), 1, 1, &mut display).is_err());
    assert!(mask.show().is_none());
    assert!(mask.publication().is_none());
}
