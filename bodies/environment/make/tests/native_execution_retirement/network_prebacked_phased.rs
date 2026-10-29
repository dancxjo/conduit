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
