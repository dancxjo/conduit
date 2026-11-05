use conduit_plot::rust_binding::NativeRustBinding;
use conduit_time::{
    CalendarDateIntent, DeadlineIntent, LocalDate, ObservanceWindowIntent, SolarLandmark,
    TemporalBoundary, TemporalInstant, TemporalLandmark,
};

fn distinct<A: NativeRustBinding, B: NativeRustBinding>() {
    assert_ne!(
        A::semantic_type().unwrap(),
        B::semantic_type().unwrap(),
        "different temporal intentions must not collapse into one Type"
    );
}

fn date(day: u8) -> CalendarDateIntent {
    CalendarDateIntent::new(
        "hebrew".into(),
        day,
        "Adar-II".into(),
        "rules/v1".into(),
        5787,
    )
    .unwrap()
}

#[test]
fn structured_decode_rechecks_untrusted_calendar_labels() {
    for (field, invalid) in [
        ("calendar_identity", serde_json::json!("")),
        ("calendar_identity", serde_json::json!("c".repeat(129))),
        ("rule_set_identity", serde_json::json!("")),
        ("rule_set_identity", serde_json::json!("r".repeat(129))),
        ("month_code", serde_json::json!("")),
        ("month_code", serde_json::json!("m".repeat(33))),
        ("day", serde_json::json!(0)),
        ("day", serde_json::json!(32)),
    ] {
        let mut wire = serde_json::to_value(date(1)).unwrap();
        wire[field] = invalid;
        // Serde is a representation adapter, not checked Native admission.
        let untrusted: CalendarDateIntent = serde_json::from_value(wire).unwrap();
        let structured = untrusted.into_structured().unwrap();
        assert!(CalendarDateIntent::from_structured(structured).is_err());
    }
}

fn round_trip<T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq>(value: T) {
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn calendar_labels_enforce_byte_and_day_bounds_without_resolving_dates() {
    for (calendar, month, rules, day) in [
        ("c".repeat(128), "m".repeat(32), "r".repeat(128), 1),
        ("é".repeat(64), "é".repeat(16), "é".repeat(64), 31),
    ] {
        round_trip(CalendarDateIntent::new(calendar, day, month, rules, i32::MIN).unwrap());
    }
    for (calendar, month, rules, day) in [
        (String::new(), "month".into(), "rules".into(), 1),
        ("c".repeat(129), "month".into(), "rules".into(), 1),
        ("é".repeat(65), "month".into(), "rules".into(), 1),
        ("calendar".into(), String::new(), "rules".into(), 1),
        ("calendar".into(), "m".repeat(33), "rules".into(), 1),
        ("calendar".into(), "month".into(), String::new(), 1),
        ("calendar".into(), "month".into(), "r".repeat(129), 1),
        ("calendar".into(), "month".into(), "rules".into(), 0),
        ("calendar".into(), "month".into(), "rules".into(), 32),
    ] {
        assert!(CalendarDateIntent::new(calendar, day, month, rules, 5787).is_err());
    }
    // A label's calendar validity belongs to a later resolver, not this Type.
    round_trip(date(31));
}

#[test]
fn deadline_and_observance_preserve_unresolved_independent_landmarks() {
    let due = TemporalLandmark::calendar_date(date(14)).unwrap();
    round_trip(due.clone());
    for boundary in [TemporalBoundary::Inclusive, TemporalBoundary::Exclusive] {
        let deadline = DeadlineIntent::new(boundary, "due".into(), due.clone()).unwrap();
        assert_eq!(deadline.latest(), &due);
        assert_eq!(deadline.boundary(), &boundary);
        assert!(matches!(
            deadline.latest(),
            TemporalLandmark::CalendarDate(_)
        ));
        round_trip(deadline);
    }
    let begins = TemporalLandmark::solar(
        "sunset-convention".into(),
        date(14),
        SolarLandmark::Sunset,
        "place/a".into(),
    )
    .unwrap();
    let ends = TemporalLandmark::solar(
        "nightfall-convention".into(),
        date(15),
        SolarLandmark::Nightfall,
        "place/b".into(),
    )
    .unwrap();
    let window = ObservanceWindowIntent::new(
        begins.clone(),
        "tradition/v1".into(),
        ends.clone(),
        "window".into(),
        "occasion".into(),
    )
    .unwrap();
    assert_eq!(window.begins(), &begins);
    assert_eq!(window.ends(), &ends);
    assert_ne!(window.begins(), window.ends());
    assert!(matches!(window.begins(), TemporalLandmark::Solar(_)));
    assert!(matches!(window.ends(), TemporalLandmark::Solar(_)));
    round_trip(window);
    round_trip(begins);
    round_trip(ends);
}

#[test]
fn solar_and_normative_identities_refuse_empty_and_overlong_values() {
    let anchor = TemporalLandmark::calendar_date(date(1)).unwrap();
    for identity in [String::new(), "i".repeat(257), "é".repeat(129)] {
        assert!(TemporalLandmark::solar(
            identity.clone(),
            date(1),
            SolarLandmark::Sunrise,
            "place".into()
        )
        .is_err());
        assert!(TemporalLandmark::solar(
            "convention".into(),
            date(1),
            SolarLandmark::Sunrise,
            identity.clone()
        )
        .is_err());
        assert!(DeadlineIntent::new(
            TemporalBoundary::Inclusive,
            identity.clone(),
            anchor.clone()
        )
        .is_err());
        for field in 0..3 {
            let mut identities = ["convention".into(), "window".into(), "occasion".into()];
            identities[field] = identity.clone();
            let [convention, window, occasion] = identities;
            assert!(ObservanceWindowIntent::new(
                anchor.clone(),
                convention,
                anchor.clone(),
                window,
                occasion
            )
            .is_err());
        }
    }
    let maximum = "é".repeat(128);
    let solar = TemporalLandmark::solar(
        maximum.clone(),
        date(1),
        SolarLandmark::Sunrise,
        maximum.clone(),
    )
    .unwrap();
    round_trip(
        DeadlineIntent::new(TemporalBoundary::Exclusive, maximum.clone(), solar.clone()).unwrap(),
    );
    round_trip(
        ObservanceWindowIntent::new(
            solar.clone(),
            maximum.clone(),
            solar,
            maximum.clone(),
            maximum,
        )
        .unwrap(),
    );
}

#[test]
fn unresolved_intents_have_distinct_checked_types() {
    // A non-Gregorian date label is not a validated LocalDate or an instant.
    distinct::<CalendarDateIntent, LocalDate>();
    distinct::<CalendarDateIntent, TemporalInstant>();
    distinct::<TemporalLandmark, TemporalInstant>();

    // A normative deadline is not an observance window or a clock event.
    distinct::<DeadlineIntent, ObservanceWindowIntent>();
    distinct::<DeadlineIntent, TemporalInstant>();

    // This test validates Conduitese-owned bindings. No time/place resolution
    // or runtime support is implied by the presence of a portable Type.
}

#[test]
fn solar_landmark_vocabulary_round_trips_without_a_host() {
    for landmark in [
        SolarLandmark::Sunrise,
        SolarLandmark::Sunset,
        SolarLandmark::Nightfall,
        SolarLandmark::SolarNoon,
    ] {
        let structured = landmark.into_structured().unwrap();
        assert_eq!(
            structured.value_type(),
            &SolarLandmark::semantic_type().unwrap()
        );
        assert_eq!(
            SolarLandmark::from_structured(structured).unwrap(),
            landmark
        );
    }
}
