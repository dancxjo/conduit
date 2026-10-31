use conduit_core::{Quantity, Unit};
use conduit_environment_history::RetainedEvidenceGraph;
use conduit_environment_runtime::{
    observed_record::ObservedRecord, recording::RecordingDisposition,
};
fn record(sequence: u64, time: u64, available: bool) -> [u8; 21] {
    ObservedRecord {
        sequence,
        observed_at_boot_ms: time,
        disposition: if available {
            RecordingDisposition::Available
        } else {
            RecordingDisposition::Checksum
        },
        quantities: available.then_some((
            Quantity::new(20, Unit::Celsius),
            Quantity::new(50, Unit::Percent),
        )),
    }
    .encode()
    .unwrap()
}

#[test]
fn visible_time_window_uses_original_timestamps_without_mutating_samples() {
    let bytes = [
        record(1, 1_000, true),
        record(2, 80_000, false),
        record(4, 100_000, true),
    ]
    .concat();
    let graph = RetainedEvidenceGraph::prepare(&bytes, 4, 0).unwrap();
    let original: Vec<_> = graph
        .samples()
        .iter()
        .map(|s| (s.sequence(), s.boot_ms(), s.failure(), s.gap_before()))
        .collect();
    assert_eq!(
        graph
            .samples_in_window(Some(60_000))
            .iter()
            .map(|s| s.sequence())
            .collect::<Vec<_>>(),
        vec![2, 4]
    );
    for width in [320, 644, 1024] {
        let scene = graph
            .native_scene_in_unit_at_size_with_window(Unit::Fahrenheit, width, 476, Some(60_000))
            .unwrap();
        assert!(scene.commands().iter().any(|c| c.payload().contains("°F")));
        assert!(scene.commands().iter().all(|c| c.bounds.x >= 0
            && i32::from(c.bounds.x) + i32::from(c.bounds.width) <= i32::from(width)));
    }
    assert_eq!(
        graph
            .samples()
            .iter()
            .map(|s| (s.sequence(), s.boot_ms(), s.failure(), s.gap_before()))
            .collect::<Vec<_>>(),
        original
    );
}
