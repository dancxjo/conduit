use conduit_form::rust_binding::NativeRustBinding;
use conduit_time::{
    HistoricalEntryOrigin, HistoricalRetentionGap, HistoricalTimelineCommand,
    HistoricalTimelineEntry, HistoricalTimelineOutcome, NativeTemporalInstant,
};

mod common;

fn instant() -> NativeTemporalInstant {
    conduit_core::TemporalInstant {
        ticks: 42,
        scale: conduit_core::TemporalScale::Milliseconds,
        clock_basis: "history-clock".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
    .try_into()
    .unwrap()
}

#[test]
fn historical_entry_gap_command_and_outcome_are_native() {
    let value = common::replay_value(5, "bench/record@1");
    let entry = HistoricalTimelineEntry::new(
        instant(),
        "observation/amber".into(),
        HistoricalEntryOrigin::MachineObservation,
        7,
        value.clone(),
    )
    .unwrap();
    assert_eq!(
        HistoricalTimelineEntry::from_structured(entry.clone().into_structured().unwrap()),
        Ok(entry.clone())
    );

    let gap = HistoricalRetentionGap::new(2, 3, 4, 8).unwrap();
    assert_eq!(
        HistoricalRetentionGap::from_structured(gap.into_structured().unwrap()),
        Ok(gap)
    );

    let command = HistoricalTimelineCommand::append(
        instant(),
        "observation/amber".into(),
        HistoricalEntryOrigin::MachineObservation,
        value,
    )
    .unwrap();
    assert_eq!(
        HistoricalTimelineCommand::from_structured(command.clone().into_structured().unwrap()),
        Ok(command)
    );

    let outcome = HistoricalTimelineOutcome::removed(
        entry.event_time,
        entry.identity,
        entry.origin,
        entry.sequence,
        entry.value,
    )
    .unwrap();
    assert_eq!(
        HistoricalTimelineOutcome::from_structured(outcome.clone().into_structured().unwrap()),
        Ok(outcome)
    );
}
