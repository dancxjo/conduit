#[path = "common/identity.rs"]
mod common;
use conduit_core::{ActivePlayId, BootId, HostId, PlanId};
use conduit_dht11_observation::ObservationContract;
use conduit_environment_history::{HistoryFaceRefusal, HistoryGraph, PreparedHistoryFace};
use conduit_environment_runtime::recording::{
    PreparedRecordingChunk, RecordingContext, RecordingDisposition,
};
use conduit_face::{Face, FaceBasis};

#[test]
fn two_producer_boots_keep_disjoint_subjects_and_independent_time_domains() {
    let contract = ObservationContract::prepare().unwrap();
    let mut fragments = Vec::new();
    for (label, times) in [("blue", [1000, 11000]), ("yellow", [9000, 9001])] {
        let host = HostId::from(format!("host/{label}"));
        let boot = BootId::from(format!("boot/{label}"));
        let plan = PlanId::from(format!("plan/{label}"));
        let play = ActivePlayId::from(format!("play/{label}"));
        let context = || RecordingContext {
            host: &host,
            boot: &boot,
            plan: &plan,
            play: &play,
        };
        let subject = format!("history/{label}");
        let mut history =
            PreparedHistoryFace::prepare_for_subject(context(), &contract, 1, &subject).unwrap();
        let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
        for (index, time) in times.into_iter().enumerate() {
            chunk
                .append(index as u64 + 1, time, RecordingDisposition::Checksum, None)
                .unwrap();
        }
        history.read_next(chunk.seal().unwrap()).unwrap();
        fragments.push(
            history
                .series_contribution(
                    common::basis(),
                    conduit_environment_history::HistoryFaceWindow::complete_chunk(),
                )
                .unwrap(),
        );
    }
    let mut first = fragments.remove(0);
    let second = fragments.remove(0);
    first.subjects.extend(second.subjects);
    first.relationships.extend(second.relationships);
    first.properties.extend(second.properties);
    let basis = common::basis();
    let face = Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/two-producer-fixture".into()),
            checked_plot_id: Some(basis.checked_plot_id),
            expanded_plot_id: Some("expanded/two-producer-fixture".into()),
            plan_id: Some(basis.plan_id),
            active_play_id: Some(basis.active_play_id),
            sign_ids: vec![],
        },
        first.subjects,
        first.relationships,
        first.properties,
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    for (label, times) in [("blue", [1000, 11000]), ("yellow", [9000, 9001])] {
        let subject = format!("history/{label}");
        let graph = HistoryGraph::prepare(&face, &subject).unwrap();
        assert_eq!(graph.producer_context()[0], format!("host/{label}"));
        assert_eq!(graph.producer_boot(), format!("boot/{label}"));
        assert_eq!(
            graph
                .samples()
                .iter()
                .map(|sample| sample.boot_ms())
                .collect::<Vec<_>>(),
            times
        );
        assert_eq!(graph.x(&graph.samples()[0]), 32);
        assert_eq!(graph.x(&graph.samples()[1]), 592);
        let series = conduit_face::PreparedQuantitySeries::prepare(
            &face,
            &format!("history/{label}/series/temperature"),
            16,
        )
        .unwrap();
        assert_eq!(series.producer()[1], format!("boot/{label}"));
        assert_eq!(series.time_millionths(1), Some(1_000_000));
    }
}
#[test]
fn subject_bound_reserves_the_full_u64_sample_suffix_before_preparation() {
    let contract = ObservationContract::prepare().unwrap();
    for subject in [
        String::new(),
        "x".repeat(conduit_face::MAX_FACE_ID_BYTES - 27),
    ] {
        assert!(matches!(
            PreparedHistoryFace::prepare_for_subject(common::context(), &contract, 1, &subject),
            Err(HistoryFaceRefusal::Subject)
        ));
    }
    assert!(PreparedHistoryFace::prepare_for_subject(
        common::context(),
        &contract,
        1,
        &"x".repeat(conduit_face::MAX_FACE_ID_BYTES - 28)
    )
    .is_ok());
}
