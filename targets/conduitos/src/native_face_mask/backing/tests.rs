use super::*;
use crate::native_face_mask::tests::{CountingDisplay, fixture};
use conduit_conduitos_mask_offer::NativeMaskStorageProfile;
use conduit_kernel::SignSink;

#[test]
fn retained_store_bound_refuses_before_installation_and_does_not_enlarge() {
    let (mut mask, _) = fixture();
    let required =
        RetainedNativeMaskStorage::required_store_bytes(NativeMaskStorageProfile::Standard)
            .unwrap();
    assert!(matches!(
        mask.initialize_retained_backing(required - 1),
        Err(NativeFaceMaskError::Mask(NativeMaskPlayError::Pressure))
    ));
    assert!(!mask.retained_backing);
    assert!(mask.retained_storage.is_none());
    let actual = mask.initialize_retained_backing(required).unwrap();
    assert_eq!(actual.managed_store_bytes, required);
    assert!(actual.value_storage_bytes > 0);
    assert!(mask.initialize_retained_backing(required).is_err());
}

#[test]
fn real_mask_reuses_one_kernel_only_after_full_previous_evidence_transfer() {
    let (mut mask, mut draft) = fixture();
    let required =
        RetainedNativeMaskStorage::required_store_bytes(NativeMaskStorageProfile::Standard)
            .unwrap();
    mask.initialize_retained_backing(required).unwrap();
    let face = draft.face(&mask.birth_basis("arrival/retained")).unwrap();
    let mut display = CountingDisplay::default();
    mask.present_read_only(face.clone(), 1, 1, &mut display)
        .unwrap();
    let pointer = mask.session.as_ref().unwrap().backing_identity();
    let first = mask.show().unwrap().show.clone();
    let writes = display.writes;
    assert!(matches!(
        mask.present_read_only(face.clone(), 2, 2, &mut display),
        Err(NativeFaceMaskError::Mask(NativeMaskPlayError::Pressure))
    ));
    assert_eq!(display.writes, writes);
    assert!(matches!(
        mask.suspend(),
        Err(NativeFaceMaskError::Mask(NativeMaskPlayError::Pressure))
    ));
    assert_eq!(mask.show().unwrap().show, first);
    let evidence = mask.take_retired_evidence().unwrap().unwrap();
    assert!(evidence.signs.len() > 0);
    assert_eq!(evidence.receipt.kernel_signs, evidence.signs.len());
    assert!(mask.take_retired_evidence().unwrap().is_none());
    draft
        .edit_name(draft.revision(), "Continuing house".into())
        .unwrap();
    let next_face = draft.face(&mask.birth_basis("arrival/retained")).unwrap();
    mask.present_read_only(next_face, 2, 2, &mut display)
        .unwrap();
    assert_eq!(mask.session.as_ref().unwrap().backing_identity(), pointer);
    let second = mask.show().unwrap().show.clone();
    assert_eq!(second.plan_id, first.plan_id);
    assert_ne!(second.active_play_id, first.active_play_id);
    let second_evidence = mask.take_retired_evidence().unwrap().unwrap();
    assert_eq!(
        second_evidence.receipt.active_play_id,
        second.active_play_id
    );
    // The first complete log remains owned by the caller, not silently cleared.
    assert!(evidence.signs.len() > 0);
}

#[test]
fn cancelled_child_does_not_silently_recreate_retained_backing() {
    let (mut mask, draft) = fixture();
    let required =
        RetainedNativeMaskStorage::required_store_bytes(NativeMaskStorageProfile::Standard)
            .unwrap();
    mask.initialize_retained_backing(required).unwrap();
    let face = draft
        .face(&mask.birth_basis("arrival/cancel-retained"))
        .unwrap();
    let mut display = CountingDisplay::default();
    mask.present_read_only(face.clone(), 1, 1, &mut display)
        .unwrap();
    drop(mask.session.take()); // Drop cancels the still-pending interaction Host Call.
    assert!(mask.take_retired_evidence().unwrap().is_none());
    assert!(matches!(
        mask.present_read_only(face, 2, 2, &mut display),
        Err(NativeFaceMaskError::Mask(NativeMaskPlayError::Pressure))
    ));
}

#[test]
fn failed_output_retires_backing_without_reconstruction_or_show_receipt() {
    struct Lost;
    impl PixelTarget for Lost {
        fn format(&self) -> crate::display::DisplayFormat {
            CountingDisplay::default().format()
        }
        fn write_pixel(
            &mut self,
            _: u32,
            _: u32,
            _: u32,
        ) -> Result<(), crate::display::DisplayError> {
            Err(crate::display::DisplayError::Lost)
        }
    }
    let (mut mask, draft) = fixture();
    let required =
        RetainedNativeMaskStorage::required_store_bytes(NativeMaskStorageProfile::Standard)
            .unwrap();
    mask.initialize_retained_backing(required).unwrap();
    let face = draft
        .face(&mask.birth_basis("arrival/lost-retained"))
        .unwrap();
    assert!(matches!(
        mask.present_read_only(face.clone(), 1, 1, &mut Lost),
        Err(NativeFaceMaskError::Compositor(_))
    ));
    assert!(mask.show().is_none());
    assert!(mask.retained_storage.is_none());
    assert!(mask.take_retired_evidence().unwrap().is_none());
    assert!(matches!(
        mask.present_read_only(face, 2, 2, &mut CountingDisplay::default()),
        Err(NativeFaceMaskError::Mask(NativeMaskPlayError::Pressure))
    ));
}

#[test]
fn ordinary_mask_preparation_preserves_actual_offer_generation() {
    let provider = crate::product_bases::fixture_surface_provider();
    let mut mask = NativeFaceMask::prepare(
        "host/native".into(),
        "boot/native".into(),
        conduit_core::OfferGeneration(7),
        "build",
        provider.entry.base_id.clone(),
        "surface/actual-generation",
        &provider,
    )
    .unwrap();
    let face = crate::native_face_mask::tests::draft()
        .face(&mask.birth_basis("arrival/gen-seven"))
        .unwrap();
    mask.present_read_only(face, 1, 1, &mut CountingDisplay::default())
        .unwrap();
    assert_eq!(mask.show().unwrap().show.offer_generation.0, 7);
    assert_eq!(mask.publication().unwrap().offer_generation.0, 7);
}
