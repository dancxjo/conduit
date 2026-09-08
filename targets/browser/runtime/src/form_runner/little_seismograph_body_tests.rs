use super::*;

#[test]
fn canonical_little_seismograph_and_unrelated_text_complete_in_one_body_play() {
    let request = tests::request_from_sources(&[
        include_str!("../../../../../forms/little-seismograph/main.conduit"),
        "form unrelated {\n message: text/literal(\"unrelated workload\")\n show: presentation/text\n message > show\n}\n",
    ]);
    let original = request.plan.clone();
    let (mut session, started) = prepare(request).unwrap();
    assert!(started.play.validate_for(&original));
    assert_eq!(original.forms.len(), 2);

    let mut progress = started.progress;
    let mut plot = None;
    let mut threshold = None;
    let mut unrelated = false;
    for _ in 0..20 {
        match progress {
            TourProgress::Effect(effect) => {
                let TourHostEffect::Manifestation(value) = *effect else {
                    panic!("deterministic Seismograph source must not request a Host effect")
                };
                assert_eq!(value.active_play_id, started.play.active_play_id.as_str());
                match value.presentation_kind.as_str() {
                    conduit_data::MEASUREMENT_PLOT_PRESENTATION_KIND => plot = value.text,
                    conduit_data::MEASUREMENT_THRESHOLD_PRESENTATION_KIND => threshold = value.text,
                    "presentation/text" => {
                        assert_eq!(value.text.as_deref(), Some("unrelated workload"));
                        unrelated = true;
                    }
                    _ => panic!("unexpected manifestation"),
                }
                progress = session
                    .complete_effect(
                        &value.active_play_id,
                        &value.placement_id,
                        value.observation_sequence,
                        None,
                    )
                    .unwrap();
            }
            TourProgress::Receipt(receipt) => {
                assert_eq!(receipt.disposition, "completed");
                assert_eq!(receipt.active_play_id, started.play.active_play_id.as_str());
                assert_eq!(receipt.manifestation_completions, 3);
                assert_eq!(plot.as_deref(), Some("plot 1 samples · 0 omitted"));
                assert_eq!(
                    threshold.as_deref(),
                    Some("threshold Above · Some(RoseAbove)")
                );
                assert!(unrelated);
                assert_eq!(session.fragments.len(), 2);
                return;
            }
            TourProgress::Waiting { .. } if session.pending.is_empty() => {
                progress = session.poll_effect().unwrap();
            }
            TourProgress::Waiting { .. } => {
                progress =
                    TourProgress::Effect(Box::new(session.project_pending_effect(0).unwrap()));
            }
            other => panic!("unexpected progress: {other:?}"),
        }
    }
    panic!("Little Seismograph Body Play did not complete within its finite effect bound");
}
