//! The same fixed stores survive real renderer acknowledgments and rearm.
use super::*;
use conduit_kernel::SignSink;
fn read_only_stage() -> conduit_conduitos_mask_offer::MaskStage {
    let provider = crate::product_bases::fixture_surface_provider();
    let offer = conduit_conduitos_mask_offer::native_read_only_host_advertisement(
        &"host/native-mask".into(),
        &"boot/native-mask".into(),
        1,
        &provider.entry,
        NativeMaskStorageProfile::Standard,
    );
    conduit_conduitos_mask_offer::prepare_stage_from_offer(
        conduit_conduitos_mask_offer::Adapter::Native,
        &offer,
    )
    .unwrap()
}
#[test]
fn retained_kernel_rearms_after_complete_evidence_handoff_without_reallocation() {
    let planned = read_only_stage();
    let (template, mut compositor) = fixture_scanout_with_stage(1, read_only_stage());
    let face = template.renderer_request().face().clone();
    drop(template);
    let required =
        RetainedNativeMaskStorage::required_store_bytes(NativeMaskStorageProfile::Standard)
            .unwrap();
    let mut storage =
        UnboundNativeMaskStorage::prepare(NativeMaskStorageProfile::Standard, required)
            .unwrap()
            .bind(&planned.planned_mask)
            .unwrap();
    let original = storage.backing_identity();
    let mut first_evidence = None;
    let mut first_request = None;
    for sequence in [1, 2] {
        let current_face = Face::new_with_semantics(
            sequence,
            face.basis.clone(),
            face.subjects.clone(),
            face.relationships.clone(),
            face.properties.clone(),
            face.text.clone(),
            face.actions.clone(),
            face.disclosures.clone(),
        )
        .unwrap();
        let pending = PreparedNativeMaskPlay::prepare_with_storage(
            &planned.planned_mask,
            &current_face,
            sequence,
            "front/native-mask",
            "surface/native-mask",
            "display/native-mask".into(),
            NativeMaskStorageProfile::Standard,
            storage,
        )
        .unwrap();
        let request = pending.scheduler.renderer_request();
        if let Some(previous) = first_request {
            assert_ne!(request, previous);
        } else {
            first_request = Some(request);
        }
        let staged = stage(&pending, &mut compositor);
        compositor
            .compose_frame(&mut FixturePixelTarget {
                writes: 0,
                fail: false,
            })
            .unwrap();
        let ack = compositor
            .scanout_acknowledgement(&staged, current_face.revision)
            .unwrap();
        let session = pending.complete_render(&ack).unwrap();
        let (returned, evidence) = session.retire_retained().unwrap();
        assert_eq!(returned.backing_identity(), original);
        assert_eq!(returned.backing().managed_store_bytes, required);
        assert!(evidence.signs.len() > 0);
        assert_eq!(evidence.receipt.kernel_signs, evidence.signs.len());
        if let Some(first) = &first_evidence {
            let first: &RetiredNativeMaskEvidence = first;
            assert!(first.signs.len() > 0);
            assert_ne!(
                first.receipt.active_play_id,
                evidence.receipt.active_play_id
            );
        } else {
            first_evidence = Some(evidence);
        }
        storage = returned;
    }
}
