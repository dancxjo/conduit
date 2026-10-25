mod common;
use common::*;
use conduit_core::Quantity;
use conduit_environment_face::{contribution, SensorStatus, SUBJECT};
use conduit_environment_history::{HistoryGraph, HISTORY_SUBJECT};
use conduit_face::{Face, FaceBasis, GraphicsCommandKind};
use conduit_seven_segment_mask::{
    render, MaskDisposition, PreparedEnvironmentFace, SevenSegmentLook, SevenSegmentSelection,
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
        4
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
