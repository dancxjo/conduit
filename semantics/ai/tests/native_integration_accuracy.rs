use conduit_ai::IntegrationAccuracy;
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn integration_accuracy_round_trips_exact_authored_law_boundaries() {
    for accuracy in [
        IntegrationAccuracy::new(1, 0, 1).unwrap(),
        IntegrationAccuracy::new(0, 1, u64::MAX).unwrap(),
        IntegrationAccuracy::new(u64::MAX, u64::MAX, 1).unwrap(),
    ] {
        let structured = accuracy.into_structured().unwrap();
        assert_eq!(
            IntegrationAccuracy::from_structured(structured).unwrap(),
            accuracy
        );
    }

    assert!(IntegrationAccuracy::new(0, 0, 1).is_err());
    assert!(IntegrationAccuracy::new(1, 0, 0).is_err());
}
