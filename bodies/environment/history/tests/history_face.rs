mod common;
use common::*;
use conduit_core::Quantity;
use conduit_environment_history::*;
use conduit_environment_runtime::{
    observation::ObservationContract,
    recording::{RecordingHistoryRefusal, RecordingReadRefusal},
};
use conduit_face::{FaceFragment, FacePropertyValue as V};
fn property<'a>(f: &'a FaceFragment, subject: &str, name: &str) -> Option<&'a V> {
    f.properties
        .iter()
        .find(|p| p.subject == subject && p.name == name)
        .map(|p| &p.value)
}
#[test]
fn quantities_failure_gaps_and_source_clock_survive_without_invented_currentness() {
    let mut history = prepared(3);
    history
        .read_next(&chunk(&[(7, 1000, true), (8, 1500, false)]))
        .unwrap();
    let f = history.contribution(basis()).unwrap();
    assert_eq!(f.basis.plan_id.as_str(), "plan/reader");
    assert_eq!(
        property(&f, HISTORY_SUBJECT, "producer-plan"),
        Some(&V::Identity("plan/sensor".into()))
    );
    assert_eq!(
        property(&f, HISTORY_SUBJECT, "clock"),
        Some(&V::Text("producer-boot-ms".into()))
    );
    assert_eq!(
        property(&f, HISTORY_SUBJECT, "preceding-history-known"),
        Some(&V::Flag(false))
    );
    let first = "environment-history/sample/7";
    let Some(V::TypedValue { contract, bytes }) = property(&f, first, "temperature") else {
        panic!()
    };
    contract.validate(bytes).unwrap();
    assert_eq!(
        Quantity::decode(bytes).unwrap(),
        Quantity::parse_plot_literal("19.8°C").unwrap()
    );
    assert_eq!(
        property(&f, first, "sampled-at-boot-ms"),
        Some(&V::Count(1000))
    );
    assert_eq!(property(&f, first, "fresh"), Some(&V::Flag(false)));
    let failed = "environment-history/sample/8";
    assert!(property(&f, failed, "temperature").is_none());
    assert_eq!(
        property(&f, failed, "sensor-failure"),
        Some(&V::Text("checksum".into()))
    );
    history.read_next(&chunk(&[(11, 9000, true)])).unwrap();
    let f = history.contribution(basis()).unwrap();
    assert_eq!(
        property(&f, HISTORY_SUBJECT, "recovered-samples"),
        Some(&V::Count(3))
    );
    assert_eq!(
        property(&f, HISTORY_SUBJECT, "samples-outside-view"),
        Some(&V::Count(2))
    );
    let last = "environment-history/sample/11";
    assert_eq!(
        property(&f, last, "missing-sequence-first"),
        Some(&V::Count(9))
    );
    assert_eq!(
        property(&f, last, "missing-sequence-last"),
        Some(&V::Count(10))
    );
    assert_eq!(f.subjects.len(), 2);
}
#[test]
fn replay_foreign_context_clock_corruption_and_capacity_preserve_visible_history() {
    let mut history = prepared(2);
    let original = chunk(&[(7, 1000, true)]);
    history.read_next(&original).unwrap();
    let before = history.contribution(basis()).unwrap();
    for invalid in [
        original.clone(),
        chunk(&[(8, 999, true)]),
        chunk(&[(9, 2000, true)]).replace("626f6f742f74657374", "626f6f742f6e657874"),
        "{}".into(),
    ] {
        assert!(history.read_next(&invalid).is_err());
        assert_eq!(history.contribution(basis()).unwrap(), before);
    }
    history.read_next(&chunk(&[(9, 2000, false)])).unwrap();
    let full = history.contribution(basis()).unwrap();
    assert_eq!(
        history.read_next(&chunk(&[(10, 3000, true)])),
        Err(HistoryFaceRefusal::Recording(
            RecordingHistoryRefusal::Capacity
        ))
    );
    assert_eq!(history.contribution(basis()).unwrap(), full);
    let mut replay = prepared(2);
    replay.read_next(&original).unwrap();
    assert_eq!(
        replay.read_next(&original),
        Err(HistoryFaceRefusal::Recording(
            RecordingHistoryRefusal::Content(RecordingReadRefusal::Sequence)
        ))
    );
}
#[test]
fn empty_and_maximum_views_are_bounded_and_repeated_inspection_does_not_advance() {
    assert!(
        PreparedHistoryFace::prepare(context(), &ObservationContract::prepare().unwrap(), 0)
            .is_err()
    );
    let mut history = prepared(1);
    let empty = history.contribution(basis()).unwrap();
    assert_eq!(empty.subjects.len(), 1);
    assert_eq!(
        property(&empty, HISTORY_SUBJECT, "visible-samples"),
        Some(&V::Count(0))
    );
    history
        .read_next(&chunk(
            &(1..=64)
                .map(|n| (n, n * 1000, n % 2 == 0))
                .collect::<Vec<_>>(),
        ))
        .unwrap();
    let full = history.contribution(basis()).unwrap();
    assert_eq!(full.subjects.len(), 65);
    assert_eq!(history.recovered().samples, 64);
    let sealed = conduit_face::Face::new_with_semantics(
        1,
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/history".into()),
            checked_plot_id: Some(basis().checked_plot_id),
            expanded_plot_id: Some("expanded/history".into()),
            plan_id: Some(basis().plan_id),
            active_play_id: Some(basis().active_play_id),
            sign_ids: vec![],
        },
        full.subjects.clone(),
        full.relationships.clone(),
        full.properties.clone(),
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    assert!(sealed.content_bytes() <= conduit_face::MAX_FACE_TOTAL_BYTES);
    let graph = HistoryGraph::prepare(&sealed, HISTORY_SUBJECT).unwrap();
    assert_eq!(graph.samples().len(), 16);
    assert_eq!(graph.omitted(), 48);
    assert!(graph.native_scene().unwrap().commands().len() <= 48);
    assert!(graph.browser_html().len() < 24 * 1024);
    assert_eq!(history.contribution(basis()).unwrap(), full);
    assert_eq!(history.recovered().samples, 64);
}

#[test]
fn explicit_tail_window_retains_full_recovery_and_boundary_failure_and_gap() {
    assert_eq!(
        HistoryFaceWindow::prepare(0),
        Err(HistoryFaceRefusal::Window)
    );
    assert_eq!(
        HistoryFaceWindow::prepare(65),
        Err(HistoryFaceRefusal::Window)
    );
    let window = HistoryFaceWindow::prepare(8).unwrap();
    let mut history = prepared(2);
    assert_eq!(
        history
            .contribution_window(basis(), window)
            .unwrap()
            .subjects
            .len(),
        1
    );
    let events = (1..=64)
        .map(|n| (if n >= 57 { n + 2 } else { n }, n * 1000, n != 57))
        .collect::<Vec<_>>();
    history.read_next(&chunk(&events)).unwrap();
    let complete = history.contribution(basis()).unwrap();
    let tail = history.contribution_window(basis(), window).unwrap();
    assert_eq!(tail.subjects.len(), 9);
    for (name, value) in [
        ("recovered-samples", 64),
        ("visible-samples", 8),
        ("samples-outside-view", 56),
    ] {
        assert_eq!(
            property(&tail, HISTORY_SUBJECT, name),
            Some(&V::Count(value))
        );
    }
    let first = "environment-history/sample/59";
    assert_eq!(
        property(&tail, first, "missing-sequence-first"),
        Some(&V::Count(57))
    );
    assert_eq!(
        property(&tail, first, "missing-sequence-last"),
        Some(&V::Count(58))
    );
    assert_eq!(
        property(&tail, first, "sensor-failure"),
        Some(&V::Text("checksum".into()))
    );
    assert!(property(&tail, first, "temperature").is_none());
    let sealed = conduit_face::Face::new_with_semantics(
        1,
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/history".into()),
            checked_plot_id: Some(basis().checked_plot_id),
            expanded_plot_id: Some("expanded/history".into()),
            plan_id: Some(basis().plan_id),
            active_play_id: Some(basis().active_play_id),
            sign_ids: vec![],
        },
        tail.subjects.clone(),
        tail.relationships.clone(),
        tail.properties.clone(),
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let graph = HistoryGraph::prepare(&sealed, HISTORY_SUBJECT).unwrap();
    assert_eq!(graph.omitted(), 56);
    assert_eq!(graph.samples().len(), 8);
    assert_eq!(graph.samples()[0].gap_before(), Some((57, 58)));
    assert!(graph.samples()[0].failure());
    assert_eq!(history.contribution_window(basis(), window).unwrap(), tail);
    assert_eq!(history.contribution(basis()).unwrap(), complete);
    assert_eq!(history.recovered().samples, 64);
    history.read_next(&chunk(&[(67, 65000, true)])).unwrap();
    let next = history.contribution_window(basis(), window).unwrap();
    assert_eq!(
        property(&next, HISTORY_SUBJECT, "samples-outside-view"),
        Some(&V::Count(64))
    );
    assert_eq!(
        property(&next, HISTORY_SUBJECT, "recovered-samples"),
        Some(&V::Count(65))
    );
}
