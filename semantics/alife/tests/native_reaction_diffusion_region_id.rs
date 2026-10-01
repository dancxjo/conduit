use conduit_alife::ReactionDiffusionRegionId;
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn region_identity_has_exact_native_round_trips_and_full_u16_domain() {
    for raw in [0, 10, u16::MAX] {
        let identity = ReactionDiffusionRegionId::new(raw).unwrap();
        assert_eq!(identity.get(), &raw);
        let structured = identity.into_structured().unwrap();
        assert_eq!(
            ReactionDiffusionRegionId::from_structured(structured).unwrap(),
            identity
        );
    }
}
