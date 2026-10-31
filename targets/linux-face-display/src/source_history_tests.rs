//! Component proofs, not physical or live-feed acceptance.
use conduit_core::{Quantity, Unit};
use conduit_environment_history::source_series::*;
use conduit_environment_measurement::{EnvironmentalReading, source_observation::*};
use conduit_face::{
    Face, FaceBasis, FaceContributionBasis, FaceProperty, FacePropertyValue as V, FaceRole,
    FaceSubject,
};
fn fixture(count: usize) -> Face {
    let c = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&c).unwrap();
    let bytes: Vec<_> = (1..=count)
        .map(|i| {
            codec
                .encode(SourceObservation {
                    observed_at_host_boot_ms: i as u64 * 1000,
                    outcome: SourceOutcome::Available(SourceSample {
                        reading: EnvironmentalReading::temperature_humidity(
                            Quantity::new(20, Unit::Celsius),
                            Quantity::new(50, Unit::Percent),
                        )
                        .unwrap(),
                        source_counter: i as u64,
                        reported_source_ms: None,
                    }),
                })
                .unwrap()
                .to_vec()
        })
        .collect();
    let records: Vec<_> = bytes
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSeriesRecord {
            sequence: i as u64 + 1,
            observation: b,
        })
        .collect();
    let series: Vec<_> = [
        ("Blue", "series/blue", "equipment/blue"),
        ("Green", "series/green", "equipment/green"),
        ("White", "series/white", "equipment/white"),
        ("Yellow", "series/yellow", "equipment/yellow"),
    ]
    .into_iter()
    .map(|(name, subject, source)| SourceSeries {
        name,
        subject,
        source,
        stale_after_ms: 5000,
        samples_outside_view: 0,
        records: &records,
    })
    .collect();
    let basis = FaceContributionBasis {
        checked_plot_id: "checked/source-history".into(),
        plan_id: "plan/source-history".into(),
        active_play_id: "play/source-history".into(),
        required_interaction_context: None,
    };
    let mut fragment = SourceSeriesCollection {
        subject: "observations",
        collector_host: "host/collector",
        collector_boot: "boot/collector",
        observed_through_boot_ms: count as u64 * 1000 + 100,
        series: &series,
    }
    .contribution(basis.clone())
    .unwrap();
    fragment.subjects.push(FaceSubject {
        identity: "environment".into(),
        role: FaceRole::Info,
        name: "Room conditions".into(),
    });
    fragment.properties.push(FaceProperty {
        subject: "environment".into(),
        name: "sensor-status".into(),
        value: V::Text("available".into()),
    });
    Face::new(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/source-history".into()),
            checked_plot_id: Some(basis.checked_plot_id),
            expanded_plot_id: Some("expanded/source-history".into()),
            plan_id: Some(basis.plan_id),
            active_play_id: Some(basis.active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        vec![],
    )
    .unwrap()
}
#[test]
fn actual_font_metrics_and_split_report_keep_all_series_and_original_details() {
    let face = fixture(16);
    let original = face.clone();
    for look in [
        conduit_face::field_instrument_day_look().unwrap(),
        conduit_face::field_instrument_night_look().unwrap(),
    ] {
        let prepared =
            crate::source_history::prepare(&face, Unit::Fahrenheit, 500, 440, None, Some(&look))
                .unwrap()
                .unwrap();
        for name in ["Blue", "Green", "White", "Yellow"] {
            assert!(
                prepared
                    .scene
                    .commands()
                    .iter()
                    .any(|c| c.payload() == name)
            );
        }
        let (report, _, pages, _, chart) = crate::view::frame_with_context(
            face.clone(),
            500,
            476,
            0,
            Some(Unit::Fahrenheit),
            false,
            Some(&look),
            false,
            false,
        )
        .unwrap();
        assert_eq!(pages, 1);
        assert!(!chart);
        assert!(
            report
                .commands()
                .iter()
                .any(|c| c.payload() == "Room conditions")
        );
        assert!(
            !report
                .commands()
                .iter()
                .any(|c| c.payload().starts_with("Observation "))
        );
        // The complete shared Face remains accepted by exact Details.
        assert!(
            crate::view::frame_with_context(
                face.clone(),
                500,
                476,
                0,
                Some(Unit::Fahrenheit),
                true,
                Some(&look),
                false,
                false
            )
            .is_ok()
        );
    }
    assert_eq!(face, original);
}
#[test]
fn unsplit_cycles_report_and_shared_graph_without_technical_history_pages() {
    let face = fixture(2);
    let (graph, page, pages, _, chart) = crate::view::frame_with_context(
        face.clone(),
        1024,
        476,
        0,
        Some(Unit::Fahrenheit),
        false,
        None,
        false,
        true,
    )
    .unwrap();
    assert_eq!((page, pages, chart), (0, 2, true));
    assert!(graph.commands().iter().any(|c| c.payload() == "Blue"));
    let (report, page, pages, _, chart) = crate::view::frame_with_context(
        face.clone(),
        1024,
        476,
        1,
        Some(Unit::Fahrenheit),
        false,
        None,
        false,
        true,
    )
    .unwrap();
    assert_eq!((page, pages, chart), (1, 2, false));
    assert!(
        report
            .commands()
            .iter()
            .any(|c| c.payload() == "Room conditions")
    );
}
#[test]
fn viewport_window_is_common_and_does_not_reinterpret_sample_times() {
    let face = fixture(16);
    let before = face.clone();
    let full = crate::source_history::prepare(&face, Unit::Fahrenheit, 500, 440, None, None)
        .unwrap()
        .unwrap();
    let tail = crate::source_history::prepare(&face, Unit::Fahrenheit, 500, 440, Some(2000), None)
        .unwrap()
        .unwrap();
    assert_ne!(full.scene.encode(), tail.scene.encode());
    assert_eq!(face, before);
    let scope = crate::source_history::scope(&face).unwrap();
    assert_eq!(scope.len(), 1 + 4 + 64);
    assert!(!scope.iter().any(|s| s == "environment"));
}
