//! The external actual storage path preserves the ordinary phased admission gate.
use super::*;
#[test]
fn original_external_storage_is_charged_and_no_allocation_follows_admission() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let admitted = core::cell::Cell::new(None);
    let ((result, pointer), memory) = allocation_probe::observe(|| {
        let storage = NativePlay::reserve_linked_backing().unwrap();
        let pointer = storage.as_ptr() as usize;
        let result = play.install_linked_phased_held_with_storage(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            storage,
            || {
                admitted.set(allocation_probe::current_observation());
                Ok(())
            },
        );
        (result, pointer)
    });
    result.unwrap();
    assert_eq!(
        pointer,
        play.prepared.as_ref().unwrap().as_ref() as *const _ as usize
    );
    let before = admitted.get().unwrap();
    assert_eq!(
        (before.allocations, before.reallocations),
        (memory.allocations, memory.reallocations)
    );
    println!(
        "external original backing: total_peak={} retained={}",
        memory.peak_bytes,
        memory.retained_bytes()
    );
    assert!(memory.peak_bytes <= 8192);
    assert_eq!(play.active.as_ref().unwrap().play_sequence, 1);
}
#[test]
fn original_external_storage_does_not_bypass_stale_scope_or_admission_refusal() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    for stale in [true, false] {
        let mut expected = pins.clone();
        if stale {
            expected.checked_plot_id = "plot/substituted".into();
        }
        let admitted = core::cell::Cell::new(false);
        let (result, memory) = allocation_probe::observe(|| {
            let storage = NativePlay::reserve_linked_backing().unwrap();
            play.install_linked_phased_held_with_storage(
                &image,
                &expected,
                c::OfferGeneration(u64::MAX),
                &mut source,
                &mut join,
                &mut issuer,
                storage,
                || {
                    admitted.set(true);
                    Err(())
                },
            )
        });
        assert!(result.is_err());
        assert_eq!(
            memory.retained_bytes(),
            0,
            "refusal retires original unused storage"
        );
        assert_eq!(admitted.get(), !stale);
        assert!(!play.attempted && play.active.is_none() && play.flat_refusal.is_empty());
        assert!(source.is_some() && join.is_some());
    }
    let storage = NativePlay::reserve_linked_backing().unwrap();
    play.install_linked_phased_held_with_storage(
        &image,
        &pins,
        c::OfferGeneration(u64::MAX),
        &mut source,
        &mut join,
        &mut issuer,
        storage,
        || Ok(()),
    )
    .unwrap();
    assert_eq!(play.active.as_ref().unwrap().play_sequence, 1);
}

#[test]
fn kernel_only_backing_defers_full_sign_grant_and_reuses_original_allocation() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let admitted = core::cell::Cell::new(None);
    let ((result, pointer), memory) = allocation_probe::observe(|| {
        let before = allocation_probe::current_observation().unwrap();
        let backing = NativePlay::reserve_linked_kernel_backing().unwrap();
        let early = allocation_probe::current_observation().unwrap();
        assert_eq!(early.allocations - before.allocations, 1);
        assert_eq!(
            early.requested_bytes - before.requested_bytes,
            n::PreparedNetworkKernelBacking::allocation_bytes()
        );
        let pointer = backing.as_ptr() as usize;
        let result = play.install_linked_phased_held_with_backing(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            backing,
            || {
                admitted.set(allocation_probe::current_observation());
                Ok(())
            },
        );
        (result, pointer)
    });
    result.unwrap();
    assert_eq!(
        pointer,
        play.prepared.as_ref().unwrap().as_ref() as *const _ as usize
    );
    let admitted = admitted.get().unwrap();
    assert_eq!(
        (admitted.allocations, admitted.reallocations),
        (memory.allocations, memory.reallocations)
    );
    assert_eq!(play.phase, "installed-held");
    assert_eq!(
        play.prepared
            .as_ref()
            .unwrap()
            .kernel
            .signs()
            .reserved_storage_bytes(),
        n::CONTROL_SIGN_BACKING_BYTES
    );
    assert_eq!(play.active.as_ref().unwrap().play_sequence, 1);
    println!(
        "delayed Signs: kernel={} signs={} CPU_peak={} CPU_retained={}",
        n::PreparedNetworkKernelBacking::allocation_bytes(),
        n::CONTROL_SIGN_BACKING_BYTES,
        memory.peak_bytes,
        memory.retained_bytes()
    );
}

#[test]
fn delayed_sign_refusal_returns_both_owners_without_admission_or_issuer_sequence_spend() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let original_source = source.as_ref().unwrap().native_identity().clone();
    let original_join = join.as_ref().unwrap().native_identity().clone();
    let mut play = NativePlay::new();
    let admitted = core::cell::Cell::new(false);
    flat::refuse_next_log();
    let (result, memory) = allocation_probe::observe(|| {
        let backing = NativePlay::reserve_linked_kernel_backing().unwrap();
        play.install_linked_phased_held_with_backing(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            backing,
            || {
                admitted.set(true);
                Ok(())
            },
        )
    });
    assert!(result.is_err());
    assert!(!admitted.get());
    assert_eq!(memory.retained_bytes(), 0);
    assert_eq!(source.as_ref().unwrap().native_identity(), &original_source);
    assert_eq!(join.as_ref().unwrap().native_identity(), &original_join);
    assert!(!play.attempted && play.active.is_none() && play.flat_refusal.is_empty());
    assert!(play.prepared.is_none());
    play.install_linked_phased_held(
        &image,
        &pins,
        c::OfferGeneration(u64::MAX),
        &mut source,
        &mut join,
        &mut issuer,
        || Ok(()),
    )
    .unwrap();
    assert_eq!(play.active.as_ref().unwrap().play_sequence, 1);
}
