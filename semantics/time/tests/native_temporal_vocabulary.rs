use conduit_form::rust_binding::NativeRustBinding;
use conduit_time::{
    HistoricalEntryOrigin, HistoricalOverflowPolicy, TemporalBoundary, TemporalWindowPosition,
};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn history_and_window_vocabularies_round_trip_through_native_types() {
    for value in [
        HistoricalEntryOrigin::MachineObservation,
        HistoricalEntryOrigin::OperatorAuthored,
    ] {
        assert_round_trip(value);
    }
    for value in [
        HistoricalOverflowPolicy::Refuse,
        HistoricalOverflowPolicy::EvictOldestWithGap,
    ] {
        assert_round_trip(value);
    }
    for value in [TemporalBoundary::Inclusive, TemporalBoundary::Exclusive] {
        assert_round_trip(value);
    }
    for value in [
        TemporalWindowPosition::Before,
        TemporalWindowPosition::Within,
        TemporalWindowPosition::After,
        TemporalWindowPosition::Indeterminate,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn generated_window_vocabulary_keeps_existing_serde_spelling() {
    assert_eq!(
        serde_json::to_string(&TemporalBoundary::Inclusive).unwrap(),
        "\"Inclusive\""
    );
    assert_eq!(
        serde_json::from_str::<TemporalWindowPosition>("\"Indeterminate\"").unwrap(),
        TemporalWindowPosition::Indeterminate
    );
}
