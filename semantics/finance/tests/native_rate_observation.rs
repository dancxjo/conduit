use conduit_finance::{
    FinanceCurrency, FinanceFixedDecimal, FinanceRateObservation, FinanceRateProfile,
    FinanceRateSource,
};
use conduit_plot::rust_binding::NativeRustBinding;

fn text(length: usize) -> String {
    "x".repeat(length)
}

#[test]
fn native_rate_observation_round_trips_every_field() {
    let value = FinanceRateObservation::new(
        FinanceCurrency::Eur,
        u64::MAX,
        FinanceRateProfile::new("finance/exact-rate@1".into()).unwrap(),
        FinanceCurrency::Usd,
        FinanceFixedDecimal::new(108_250, 5).unwrap(),
        FinanceRateSource::new("fixture/reference".into()).unwrap(),
    )
    .unwrap();

    assert_eq!(value.base(), &FinanceCurrency::Eur);
    assert_eq!(value.quote(), &FinanceCurrency::Usd);
    assert_eq!(*value.observed_ticks(), u64::MAX);
    assert_eq!(value.source().get(), "fixture/reference");
    assert_eq!(value.profile().get(), "finance/exact-rate@1");

    let structured = value.clone().into_structured().unwrap();
    assert_eq!(
        FinanceRateObservation::from_structured(structured).unwrap(),
        value
    );
}

#[test]
fn rate_source_and_profile_are_nonempty_and_bounded() {
    assert!(FinanceRateSource::new(String::new()).is_err());
    assert!(FinanceRateProfile::new(String::new()).is_err());
    assert!(FinanceRateSource::new(text(256)).is_ok());
    assert!(FinanceRateProfile::new(text(256)).is_ok());
    assert!(FinanceRateSource::new(text(257)).is_err());
    assert!(FinanceRateProfile::new(text(257)).is_err());
}
