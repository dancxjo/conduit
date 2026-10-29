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

#[test]
fn native_resolution_prepares_geometry_and_glyph_roles_before_rasterization() {
    let graph = RetainedEvidenceGraph::prepare(
        &[record(1, 1000, true), record(2, 2000, true)].concat(),
        2,
        0,
    )
    .unwrap();
    let baseline = graph.native_scene_in_unit(Unit::Fahrenheit).unwrap();
    let native = graph
        .native_scene_in_unit_at_size(Unit::Fahrenheit, 1024, 564)
        .unwrap();
    assert_eq!(native.commands().len(), baseline.commands().len());
    assert_eq!(native.commands()[0].bounds.width, 1024);
    assert_eq!(native.commands()[0].bounds.height, 564);
    for (old, new) in baseline.commands().iter().zip(native.commands()) {
        assert_eq!(old.payload(), new.payload());
        assert!(i32::from(new.bounds.y) + i32::from(new.bounds.height) <= 564);
        if let Some(line) = new.polyline_geometry() {
            assert_eq!(*line.points().first().unwrap().x(), 100);
            assert_eq!(*line.points().last().unwrap().x(), 996);
            assert_eq!(new.paint, conduit_face::GraphicsPaintRole::Accent);
        }
    }
    assert!(
        graph
            .native_scene_in_unit_at_size(Unit::Fahrenheit, 0, 564)
            .is_err()
    );
    assert!(
        graph
            .native_scene_in_unit_at_size(Unit::Fahrenheit, u16::MAX, 564)
            .is_err()
    );
}

#[test]
fn nice_fahrenheit_axes_and_worst_fragmentation_stay_within_48_commands() {
    let make = |sequence, temp_tenths| {
        let mut r = ObservedRecord::decode(&record(sequence, sequence * 1000, true)).unwrap();
        r.quantities = Some((
            Quantity::from_decimal(temp_tenths, -1, Unit::Celsius).unwrap(),
            Quantity::new(50, Unit::Percent),
        ));
        r.encode().unwrap()
    };
    let graph =
        RetainedEvidenceGraph::prepare(&[make(1, 178), make(2, 182)].concat(), 2, 0).unwrap();
    let scene = graph
        .native_scene_in_unit_at_size(Unit::Fahrenheit, 1024, 564)
        .unwrap();
    for tick in ["64.0", "64.5", "65.0", "0%", "25%", "50%", "75%", "100%"] {
        assert!(
            scene.commands().iter().any(|c| c.payload() == tick),
            "missing {tick}"
        );
    }
    for value in [0, -403, 645] {
        let graph = RetainedEvidenceGraph::prepare(&make(1, value), 1, 0).unwrap();
        let scene = graph
            .native_scene_in_unit_at_size(Unit::Celsius, 1024, 564)
            .unwrap();
        assert!(scene.commands().len() <= 48);
        for point in scene
            .commands()
            .iter()
            .filter_map(|c| c.polyline_geometry())
            .flat_map(|p| p.points())
        {
            assert!(*point.y() >= 0 && *point.y() < 564);
        }
    }
    let bytes: Vec<_> = (0..16).flat_map(|i| make(i * 2 + 1, 178)).collect();
    let graph = RetainedEvidenceGraph::prepare(&bytes, 31, 0).unwrap();
    let scene = graph
        .native_scene_in_unit_at_size(Unit::Fahrenheit, 1024, 564)
        .unwrap();
    assert_eq!(scene.commands().len(), 48);
    assert_eq!(
        scene
            .commands()
            .iter()
            .filter(|c| c.paint == conduit_face::GraphicsPaintRole::Accent)
            .count(),
        32
    );
}
