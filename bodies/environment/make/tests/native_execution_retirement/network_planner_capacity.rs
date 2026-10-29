//! Allocation refusal reaches the actual borrowed planner before its consumer.
use super::*;
#[test]
fn canonical_identity_storage_refusal_precedes_plan_consumer_and_admission() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let admitted = core::cell::Cell::new(false);
    let (result, memory) = allocation_probe::observe(|| {
        let _refusal = allocation_probe::refuse_next_allocation_size(64);
        play.install_linked_phased_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            || {
                admitted.set(true);
                Ok(())
            },
        )
    });
    assert!(result.is_err());
    // Stage105 starts only inside the actual with_linked_local_plan consumer.
    assert_eq!(flat::phased_refusal(), Some((104, false, false)));
    assert!(!admitted.get() && !play.attempted && play.active.is_none());
    assert!(source.is_some() && join.is_some() && play.flat_refusal.is_empty());
    assert_eq!(memory.retained_bytes(), 0);
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
