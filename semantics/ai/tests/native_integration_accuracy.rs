use conduit_ai::{IntegrationAccuracy, IntegrationResourceEnvelope};
use conduit_plot::rust_binding::NativeRustBinding;

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

#[test]
fn integration_resources_are_intrinsically_finite_and_positive() {
    let resources =
        IntegrationResourceEnvelope::new(128, 256, 65_536, 512, 1, 2, 3, 1_024).unwrap();
    let structured = resources.into_structured().unwrap();
    assert_eq!(
        IntegrationResourceEnvelope::from_structured(structured).unwrap(),
        resources
    );

    assert!(IntegrationResourceEnvelope::new(0, 256, 2, 512, 1, 2, 3, 1_024).is_err());
    assert!(IntegrationResourceEnvelope::new(128, 256, 1, 512, 1, 2, 3, 1_024).is_err());
    assert!(IntegrationResourceEnvelope::new(128, 256, 65_537, 512, 1, 2, 3, 1_024).is_err());
    assert!(!include_str!("../src/dynamics.rs")
        .contains(concat!("pub struct ", "IntegrationResourceEnvelope")));
}
