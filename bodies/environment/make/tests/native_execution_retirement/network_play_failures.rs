//! Actual fallible descriptive mechanism-ID storage before Ready-owner movement.
use super::*;
#[test]
fn temporary_local_base_id_refusal_returns_before_any_ready_owner_moves() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let (result, memory) = allocation_probe::observe(|| {
        let _refusal =
            allocation_probe::refuse_next_allocation_size(c::LOCAL_BASE_IMPLEMENTATION_ID.len());
        play.install_linked_phased_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            || panic!("refused allocation cannot admit"),
        )
    });
    assert!(result.is_err());
    assert_eq!(flat::phased_refusal(), Some((116, false, false)));
    assert!(source.is_some() && join.is_some() && !play.attempted);
    assert_eq!(memory.retained_bytes(), 0);
}
