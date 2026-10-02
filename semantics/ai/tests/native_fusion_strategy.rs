use conduit_ai::FusionStrategy;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn fusion_strategy_round_trips_through_its_exact_native_payload_type() {
    let strategy = FusionStrategy::reciprocal_rank(60).unwrap();
    let structured = strategy.clone().into_structured().unwrap();
    assert_eq!(
        FusionStrategy::from_structured(structured).unwrap(),
        strategy
    );
}

#[test]
fn fusion_strategy_refuses_rank_constants_outside_its_native_boundary() {
    assert!(FusionStrategy::reciprocal_rank(0).is_err());
    assert!(FusionStrategy::reciprocal_rank(u16::MAX).is_ok());
}
