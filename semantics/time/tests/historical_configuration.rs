use conduit_core::{
    kind_id, BoundedResourceRef, ConfigurationEntry, ConfigurationValue, ResourceClassId,
    ResourceExtent, ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity,
    TemporalInstant, TemporalScale,
};
use conduit_time::*;

fn configuration() -> Vec<ConfigurationEntry> {
    [
        (
            "value-profile",
            ConfigurationValue::Text("bench/record@1".into()),
        ),
        (
            "clock-basis",
            ConfigurationValue::Text("bench/event-clock".into()),
        ),
        (
            "time-scale",
            ConfigurationValue::Text("microseconds".into()),
        ),
        ("maximum-entries", ConfigurationValue::U64(1)),
        ("maximum-referenced-bytes", ConfigurationValue::U64(4)),
        ("overflow-policy", ConfigurationValue::Text("refuse".into())),
        ("first-sequence", ConfigurationValue::U64(42)),
    ]
    .into_iter()
    .map(|(key, value)| ConfigurationEntry {
        key: key.into(),
        value,
    })
    .collect()
}

fn value() -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("bench/record@1"),
        access_class: ResourceClassId::from("history/value"),
        extent: ResourceExtent {
            bytes: 4,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    }
}

#[test]
fn checked_configuration_supplies_every_semantic_constructor_input() {
    let mut timeline = historical_timeline_from_configuration(&configuration()).unwrap();
    assert_eq!(
        timeline.append(
            "bench/entry/a".into(),
            TemporalInstant {
                ticks: 10,
                scale: TemporalScale::Microseconds,
                clock_basis: "bench/event-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            HistoricalEntryOrigin::MachineObservation,
            value(),
        ),
        Ok(42)
    );
    assert_eq!(
        timeline.append(
            "bench/entry/b".into(),
            TemporalInstant {
                ticks: 11,
                scale: TemporalScale::Microseconds,
                clock_basis: "bench/event-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            HistoricalEntryOrigin::MachineObservation,
            value(),
        ),
        Err(HistoricalTimelineRefusal::Full)
    );
}

#[test]
fn missing_wrong_unknown_and_out_of_bounds_configuration_remain_distinct() {
    let mut entries = configuration();
    entries.retain(|entry| entry.key != "clock-basis");
    assert!(matches!(
        historical_timeline_from_configuration(&entries),
        Err(HistoricalConfigurationRefusal::Missing("clock-basis"))
    ));

    let mut entries = configuration();
    entries
        .iter_mut()
        .find(|entry| entry.key == "maximum-entries")
        .unwrap()
        .value = ConfigurationValue::Text("one".into());
    assert!(matches!(
        historical_timeline_from_configuration(&entries),
        Err(HistoricalConfigurationRefusal::WrongType("maximum-entries"))
    ));

    let mut entries = configuration();
    entries
        .iter_mut()
        .find(|entry| entry.key == "time-scale")
        .unwrap()
        .value = ConfigurationValue::Text("fortnights".into());
    assert!(matches!(
        historical_timeline_from_configuration(&entries),
        Err(HistoricalConfigurationRefusal::UnknownTimeScale)
    ));

    let mut entries = configuration();
    entries
        .iter_mut()
        .find(|entry| entry.key == "maximum-entries")
        .unwrap()
        .value = ConfigurationValue::U64(0);
    assert!(matches!(
        historical_timeline_from_configuration(&entries),
        Err(HistoricalConfigurationRefusal::Timeline(
            HistoricalTimelineRefusal::InvalidLimits
        ))
    ));
}
