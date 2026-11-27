//! Shared semantic interpretation of the application's actual recording view.
mod common;
use conduit_core::Quantity;
use conduit_environment_history::HistoryFaceWindow;
use conduit_face::{Face, FaceBasis, PreparedQuantitySeries};
#[test]
fn application_channels_use_shared_series_without_reinterpreting_time_or_quantities() {
    let mut history = common::prepared(1);
    history
        .read_next(&common::chunk(&[
            (1, 1000, true),
            (2, 2000, false),
            (4, 11000, true),
        ]))
        .unwrap();
    let fragment = history
        .series_contribution(common::basis(), HistoryFaceWindow::complete_chunk())
        .unwrap();
    let basis = common::basis();
    let face = Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/series-recording-fixture".into()),
            checked_plot_id: Some(basis.checked_plot_id),
            expanded_plot_id: Some("expanded/series-recording-fixture".into()),
            plan_id: Some(basis.plan_id),
            active_play_id: Some(basis.active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let before = face.clone();
    for (field, expected) in [("temperature", "19.8°C"), ("relative-humidity", "59%")] {
        let series = PreparedQuantitySeries::prepare(
            &face,
            &format!("environment-history/series/{field}"),
            16,
        )
        .unwrap();
        assert_eq!(
            series.samples()[0].value,
            Some(Quantity::parse_plot_literal(expected).unwrap())
        );
        assert_eq!(series.samples()[1].value, None);
        assert_eq!(series.samples()[1].unavailable_reason, Some("checksum"));
        assert_eq!(series.samples()[2].gap_before, Some((3, 3)));
        assert_eq!(
            (0..3)
                .map(|i| series.time_millionths(i).unwrap())
                .collect::<Vec<_>>(),
            [0, 100_000, 1_000_000]
        );
        assert_eq!(series.source(), "environment-history");
    }
    assert_eq!(face, before);
}
