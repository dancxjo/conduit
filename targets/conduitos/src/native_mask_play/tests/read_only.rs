use super::*;

#[test]
fn narrowed_read_only_back_renders_and_closes_but_cannot_emit_interaction() {
    let provider = crate::product_bases::fixture_surface_provider();
    let offer = conduit_conduitos_mask_offer::native_read_only_host_advertisement(
        &"host/native-mask".into(),
        &"boot/native-mask".into(),
        1,
        &provider.entry,
        conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
    );
    let planned = || {
        conduit_conduitos_mask_offer::prepare_stage_from_offer(
            conduit_conduitos_mask_offer::Adapter::Native,
            &offer,
        )
        .unwrap()
    };
    let (pending, mut compositor) = fixture_scanout_with_stage(1, planned());
    // Actual Standard backing, not merely a PROFILE closure label. This excludes
    // Face/Plan metadata heaps, serialized buffers, scanout, allocator overhead
    // and preparation peak.
    let budget =
        storage::admit_budget(&pending.scheduler, NativeMaskStorageProfile::Standard).unwrap();
    assert_eq!(budget.value_slots, 10);
    assert_eq!(budget.reserved_value_payload_bytes, 10 * 48 * 1024);
    let managed_stores = budget
        .scheduler_inline_bytes
        .checked_add(budget.reserved_value_storage_bytes)
        .unwrap()
        .checked_add(budget.prepared_inline_bytes)
        .unwrap();
    assert!(
        managed_stores <= 1024 * 1024,
        "Standard managed stores require {managed_stores} bytes"
    );
    std::eprintln!("STANDARD_MASK_MANAGED_STORES bytes={managed_stores} budget={budget:?}");
    let staged = stage(&pending, &mut compositor);
    compositor
        .compose_frame(&mut FixturePixelTarget {
            writes: 0,
            fail: false,
        })
        .unwrap();
    let ack = compositor.scanout_acknowledgement(&staged, 1).unwrap();
    let session = pending.complete_render(&ack).unwrap();
    let interaction = fixture_interaction(&session);
    assert!(matches!(
        session.interact(interaction),
        Err(NativeMaskPlayError::Shape)
    ));
    let (pending, mut compositor) = fixture_scanout_with_stage(2, planned());
    let staged = stage(&pending, &mut compositor);
    compositor
        .compose_frame(&mut FixturePixelTarget {
            writes: 0,
            fail: false,
        })
        .unwrap();
    let ack = compositor.scanout_acknowledgement(&staged, 1).unwrap();
    assert!(
        pending
            .complete_render(&ack)
            .unwrap()
            .close_without_input()
            .unwrap()
            .kernel_signs
            > 0
    );
}
