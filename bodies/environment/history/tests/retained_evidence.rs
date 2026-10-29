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
fn original_records_keep_gaps_failures_and_never_invent_a_body_face() {
    let bytes = [
        record(1, 1000, true),
        record(2, 2000, true),
        record(3, 3000, false),
        record(5, 5000, true),
        record(6, 6000, true),
    ]
    .concat();
    let graph = RetainedEvidenceGraph::prepare(&bytes, 6, 23).unwrap();
    assert_eq!(graph.samples()[0].temperature_milli_c(), Some(20000));
    assert!(graph.samples()[2].failure());
    assert_eq!(graph.samples()[3].gap_before(), Some((4, 4)));
    let scene = graph.native_scene_in_unit(Unit::Fahrenheit).unwrap();
    assert_eq!(
        scene
            .commands()
            .iter()
            .filter(|c| c.polyline_geometry().is_some())
            .count(),
        4
    );
    assert!(scene.commands().iter().any(|c| c.payload().contains("°F")));
    assert!(
        scene
            .commands()
            .iter()
            .any(|c| c.payload().contains("RETAINED HISTORY"))
    );
    assert!(RetainedEvidenceGraph::prepare(&bytes, 7, 23).is_err());
}
#[test]
fn finite_extent_and_actual_order_refuse() {
    assert!(RetainedEvidenceGraph::prepare(&[], 0, 0).is_err());
    assert!(RetainedEvidenceGraph::prepare(&record(1, 1, true)[..20], 1, 0).is_err());
    assert!(
        RetainedEvidenceGraph::prepare(&[record(2, 2, true), record(1, 3, true)].concat(), 2, 0)
            .is_err()
    );
    assert!(
        RetainedEvidenceGraph::prepare(&[record(1, 2, true), record(2, 1, true)].concat(), 2, 0)
            .is_err()
    );
    let records: Vec<_> = (1..=17).flat_map(|s| record(s, s, true)).collect();
    assert!(RetainedEvidenceGraph::prepare(&records, 17, 0).is_err());
}
