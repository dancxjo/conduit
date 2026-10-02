use conduit_plot::rust_binding::NativeRustBinding;
use conduit_time::{
    HistoricalEntryOrigin, HistoricalReplayEntry, NativeTemporalInstant, OwnedReplayEvent,
};

mod common;

fn instant(ticks: u64) -> NativeTemporalInstant {
    conduit_core::TemporalInstant {
        ticks,
        scale: conduit_core::TemporalScale::Milliseconds,
        clock_basis: "history-clock".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
    .try_into()
    .unwrap()
}

#[test]
fn replay_entries_and_events_are_exact_native_values() {
    let value = common::replay_value(7, "bench/record@1");
    let entry = HistoricalReplayEntry::new(
        instant(1_250),
        "observation/amber".into(),
        HistoricalEntryOrigin::MachineObservation,
        73,
        value.clone(),
    )
    .unwrap();
    assert_eq!(
        HistoricalReplayEntry::from_structured(entry.clone().into_structured().unwrap()),
        Ok(entry)
    );

    let event = OwnedReplayEvent::new(
        instant(1_250),
        "observation/amber".into(),
        HistoricalEntryOrigin::MachineObservation,
        73,
        3,
        9_000,
        value,
    )
    .unwrap();
    assert_eq!(
        OwnedReplayEvent::from_structured(event.clone().into_structured().unwrap()),
        Ok(event)
    );
}
