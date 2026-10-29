mod common;
use common::*;
use conduit_core::Quantity;
use conduit_environment_face::{SUBJECT, SensorStatus, contribution};
use conduit_environment_history::{HISTORY_SUBJECT, HistoryGraph};
use conduit_face::{Face, FaceBasis, GraphicsCommandKind};
use conduit_seven_segment_mask::{
    MaskDisposition, PreparedEnvironmentFace, SevenSegmentLook, SevenSegmentSelection, render,
};
#[test]
fn recording_and_current_reading_share_one_face_across_three_masks() {
    let mut history = prepared(2);
    history
        .read_next(&chunk(&[
            (1, 1000, true),
            (2, 2000, false),
            (4, 11000, true),
        ]))
        .unwrap();
    let mut fragment = history.contribution(basis()).unwrap();
    let current = contribution(
        Some(Quantity::parse_plot_literal("19.8°C").unwrap()),
        Some(Quantity::parse_plot_literal("59%").unwrap()),
        true,
        SensorStatus::Available,
        basis(),
    )
    .unwrap();
    fragment.subjects.extend(current.subjects);
    fragment.properties.extend(current.properties);
    let face = Face::new_with_semantics(
        7,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/history-proof".into()),
            checked_plot_id: Some(basis().checked_plot_id),
            expanded_plot_id: Some("expanded/history-proof".into()),
            plan_id: Some(basis().plan_id),
            active_play_id: Some(basis().active_play_id),
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
    let original = face.clone();
    let graph = HistoryGraph::prepare(&face, HISTORY_SUBJECT).unwrap();
    assert_eq!(graph.samples().len(), 3);
    assert_eq!(graph.samples()[0].temperature_milli_c(), Some(19800));
    assert_eq!(graph.samples()[1].temperature_milli_c(), None);
    assert_eq!(graph.samples()[2].gap_before(), Some((3, 3)));
    assert_eq!(graph.x(&graph.samples()[0]), 32);
    assert_eq!(graph.x(&graph.samples()[1]), 88);
    assert_eq!(graph.x(&graph.samples()[2]), 592);
    let html = graph.browser_html();
    let native = graph.native_show().unwrap();
    assert!(!html.contains("cx=\"88\"")); // failure is not plotted at zero
    assert!(html.contains("missing sequences 3 through 3"));
    assert_eq!(native.content_id, face.identity.as_str());
    assert_eq!(native.revision, face.revision);
    assert_eq!(
        native
            .scene
            .commands()
            .iter()
            .filter(|c| c.kind == GraphicsCommandKind::Rect)
            .count(),
        5 // Four observation dots and the Look-owned background.
    );
    let current = PreparedEnvironmentFace::prepare(&face, SUBJECT).unwrap();
    let seven = render(
        &current,
        SevenSegmentSelection::Temperature,
        &SevenSegmentLook::fahrenheit(),
    );
    assert_eq!(
        seven.disposition,
        MaskDisposition::Temperature { decimal_places: 1 }
    );
    assert_eq!(face, original); // all three interpretations leave semantic quantities intact
}

#[test]
fn line_runs_stop_at_failures_and_sequence_gaps_within_fixed_scene_budget() {
    let mut history = prepared(1);
    history
        .read_next(&chunk(&[
            (1, 1000, true),
            (2, 2000, true),
            (3, 3000, false),
            (4, 4000, true),
            (6, 6000, true),
            (7, 7000, true),
        ]))
        .unwrap();
    let fragment = history.contribution(basis()).unwrap();
    let face = Face::new_with_semantics(
        5,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/history".into()),
            checked_plot_id: Some(basis().checked_plot_id),
            expanded_plot_id: Some("expanded/history".into()),
            plan_id: Some(basis().plan_id),
            active_play_id: Some(basis().active_play_id),
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
    let graph = HistoryGraph::prepare(&face, HISTORY_SUBJECT).unwrap();
    let show = graph.native_show().unwrap();
    let lines: Vec<_> = show
        .scene
        .commands()
        .iter()
        .filter_map(|c| c.polyline_geometry())
        .collect();
    assert_eq!(lines.len(), 4); // two separate pairs per series, not across failure/gap
    assert!(lines.iter().all(|line| line.points().len() == 2));
    assert!(show.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
    assert_eq!(show.content_id, face.identity.as_str());
    assert_eq!(show.revision, 5);
}
