use super::{inspection, material, producer};
use conduit_audio::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{intent_duration_grid::*, semantic::*};

#[test]
fn unresolved_and_foreign_source_durations_refuse_without_rewriting_carrier() {
    let bytes = producer::serialize();
    let inspected = inspection::inspect(&bytes).unwrap();
    let source = material::source();
    let anchor = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let basis =
        AudioSampleRateBasis::new(anchor.clone(), AudioFrameQuantization::Floor, 8000).unwrap();
    let before = inspected.intent.clone().encode().unwrap();
    assert!(matches!(
        prepare_intent_duration_grid(&inspected.intent, source.material(), &anchor, Some(&basis)),
        Err(IntentDurationGridRefusal::Unresolved {
            event: 1,
            specification: SpeechDurationSpecification::Unknown
        })
    ));
    assert_eq!(inspected.intent.clone().encode().unwrap(), before);
    let text = source.material();
    let stale = conduit_language::LanguageText::new(
        text.identity().clone(),
        text.language().clone(),
        conduit_language::LanguageTextRevisionId::new("stale".into()).unwrap(),
        text.text().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_intent_duration_grid(&inspected.intent, &stale, &anchor, Some(&basis)),
        Err(IntentDurationGridRefusal::Sources(_))
    ));
    let rate =
        AudioSampleRateBasis::new(anchor.clone(), AudioFrameQuantization::Floor, 192001).unwrap();
    assert!(matches!(
        prepare_intent_duration_grid(&inspected.intent, text, &anchor, Some(&rate)),
        Err(IntentDurationGridRefusal::Projection(_))
    ));
    assert!(AudioSampleRateBasis::new(anchor, AudioFrameQuantization::Floor, 0).is_err());
}

#[test]
fn committed_duration_grid_retains_original_carrier_at_three_rates() {
    use conduit_audio::*;
    use conduit_speech::intent_duration_grid::*;
    let original = producer::serialize();
    let mut parts: Vec<Vec<u8>> = postcard::from_bytes(&original).unwrap();
    let intent = SpeechUtteranceIntent::decode(&parts[2]).unwrap();
    // A new explicitly authored timed specimen, never a default for the six
    // unresolved states in the original conformance artifact.
    let events = intent
        .events()
        .as_slice()
        .iter()
        .cloned()
        .map(|event| match event {
            SpeechUtteranceIntentEvent::Segment(v) => SpeechUtteranceIntentEvent::segment(
                v.occurrence().clone(),
                v.phone().clone(),
                v.phoneme().clone(),
                SpeechSegmentProsodyIntent::new(
                    SpeechDurationSpecification::known(3, 1).unwrap(),
                    v.prosody().fundamental_cycle().clone(),
                    v.prosody().relative_intensity().clone(),
                )
                .unwrap(),
                v.provenance().clone(),
                v.sources().clone(),
                v.stress().clone(),
                v.word_position().clone(),
            )
            .unwrap(),
            other => other,
        });
    let timed = SpeechUtteranceIntent::new(
        material::seq(events),
        intent.inventory_id().clone(),
        intent.language().clone(),
        intent.provenance().clone(),
        intent.revision_id().clone(),
        intent.utterance_id().clone(),
    )
    .unwrap();
    parts[2] = timed.encode().unwrap();
    let bytes = postcard::to_allocvec(&parts).unwrap();
    let inspected = inspection::inspect_original(&bytes, inspection::identity(&bytes)).unwrap();
    let source = material::source();
    let anchor = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let mut grids = Vec::new();
    for rate in [8000, 16000, 48000] {
        let basis =
            AudioSampleRateBasis::new(anchor.clone(), AudioFrameQuantization::Floor, rate).unwrap();
        let grid = prepare_intent_duration_grid(
            &inspected.intent,
            source.material(),
            &anchor,
            Some(&basis),
        )
        .unwrap();
        assert!(std::ptr::eq(grid.intent(), &inspected.intent));
        assert!(std::ptr::eq(grid.text(), source.material()));
        assert_eq!(grid.basis(), &basis);
        let mut count = 0;
        for (index, step) in grid.steps().iter().enumerate() {
            count += step.result().raw().frame_count();
            assert_eq!(count, (index as u64 + 1) * rate / 3);
            assert_eq!(
                *step.result().raw().remainder_numerator(),
                (index as u64 + 1) * rate % 3
            );
            assert!(!step.executions().is_empty());
        }
        assert_eq!(grid.intent().clone().encode().unwrap(), parts[2]);
        grids.push(
            serde_json::json!({"sample_rate_hz":rate, "endpoints":grid.steps().iter().map(|step| {
            serde_json::json!({"whole_frames":step.result().raw().whole_frames(),
                "remainder_numerator":step.result().raw().remainder_numerator(),
                "denominator":step.result().fraction().denominator(),
                "source_executions":step.executions().len()})
        }).collect::<Vec<_>>()}),
        );
    }
    if let Some(path) = std::env::var_os("CONDUIT_COMMITTED_CARRIER_OUTPUT") {
        let path = std::path::PathBuf::from(path).with_extension("duration.native");
        std::fs::write(&path, &bytes).unwrap();
        let report = serde_json::json!({"proof_class":"local Source/Native/kernel commitment and Audio Source cumulative-grid projection",
            "accepted_dev":false,"identity_domain":inspection::IDENTITY_DOMAIN,
            "identity":inspection::identity(&bytes).iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "bytes":bytes.len(),"text":inspected.text,"committed_words":inspected.committed_words,
            "durations":"seven explicitly authored 1/3-second segments", "grids":grids,
            "audio_or_neural_execution":false});
        std::fs::write(
            path.with_extension("json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }
    assert!(matches!(
        prepare_intent_duration_grid(&inspected.intent, source.material(), &anchor, None),
        Err(IntentDurationGridRefusal::MissingRate)
    ));
    let foreign = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(2).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let basis = AudioSampleRateBasis::new(foreign, AudioFrameQuantization::Floor, 8000).unwrap();
    assert!(matches!(
        prepare_intent_duration_grid(&inspected.intent, source.material(), &anchor, Some(&basis)),
        Err(IntentDurationGridRefusal::ForeignAnchor)
    ));
}

#[test]
fn quantity_bounds_and_mixed_denominators_refuse_with_original_intent_intact() {
    let original = material::intent();
    let anchor = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let basis =
        AudioSampleRateBasis::new(anchor.clone(), AudioFrameQuantization::Floor, 8000).unwrap();
    let source = material::source();
    for durations in [vec![(3, 1), (4, 1)], vec![(1, u64::from(u32::MAX) + 1)]] {
        let events = original
            .events()
            .as_slice()
            .iter()
            .enumerate()
            .map(|(i, event)| {
                let SpeechUtteranceIntentEvent::Segment(v) = event else {
                    panic!("segment")
                };
                let (denominator, numerator) = durations[i.min(durations.len() - 1)];
                SpeechUtteranceIntentEvent::segment(
                    v.occurrence().clone(),
                    v.phone().clone(),
                    v.phoneme().clone(),
                    SpeechSegmentProsodyIntent::new(
                        SpeechDurationSpecification::known(denominator, numerator).unwrap(),
                        v.prosody().fundamental_cycle().clone(),
                        v.prosody().relative_intensity().clone(),
                    )
                    .unwrap(),
                    v.provenance().clone(),
                    v.sources().clone(),
                    v.stress().clone(),
                    v.word_position().clone(),
                )
                .unwrap()
            });
        let candidate = SpeechUtteranceIntent::new(
            material::seq(events),
            original.inventory_id().clone(),
            original.language().clone(),
            original.provenance().clone(),
            original.revision_id().clone(),
            original.utterance_id().clone(),
        )
        .unwrap();
        let before = candidate.clone().encode().unwrap();
        assert!(matches!(
            prepare_intent_duration_grid(&candidate, source.material(), &anchor, Some(&basis)),
            Err(IntentDurationGridRefusal::Projection(_))
        ));
        assert_eq!(candidate.encode().unwrap(), before);
    }
}

#[test]
fn boundaries_share_cumulative_grid_and_unresolved_states_keep_global_indices() {
    let original = material::intent();
    let segment = |index: usize| {
        let SpeechUtteranceIntentEvent::Segment(v) = &original.events()[index] else {
            panic!("segment")
        };
        SpeechUtteranceIntentEvent::segment(
            v.occurrence().clone(),
            v.phone().clone(),
            v.phoneme().clone(),
            SpeechSegmentProsodyIntent::new(
                SpeechDurationSpecification::known(3, 1).unwrap(),
                v.prosody().fundamental_cycle().clone(),
                v.prosody().relative_intensity().clone(),
            )
            .unwrap(),
            v.provenance().clone(),
            v.sources().clone(),
            v.stress().clone(),
            v.word_position().clone(),
        )
        .unwrap()
    };
    let SpeechUtteranceIntentEvent::Segment(first) = &original.events()[0] else {
        panic!("segment")
    };
    let boundary = |duration| {
        SpeechUtteranceIntentEvent::boundary(
            duration,
            SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap(),
            material::evidence(),
            first.sources().clone(),
        )
        .unwrap()
    };
    let intent = |events| {
        SpeechUtteranceIntent::new(
            material::seq(events),
            original.inventory_id().clone(),
            original.language().clone(),
            original.provenance().clone(),
            original.revision_id().clone(),
            original.utterance_id().clone(),
        )
        .unwrap()
    };
    let source = material::source();
    let anchor = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let basis =
        AudioSampleRateBasis::new(anchor.clone(), AudioFrameQuantization::Floor, 8000).unwrap();
    let timed = intent(vec![
        segment(0),
        boundary(SpeechDurationSpecification::known(3, 0).unwrap()),
        boundary(SpeechDurationSpecification::known(3, 1).unwrap()),
        segment(1),
    ]);
    let grid =
        prepare_intent_duration_grid(&timed, source.material(), &anchor, Some(&basis)).unwrap();
    let endpoints: Vec<_> = grid
        .steps()
        .iter()
        .map(|step| {
            (
                *step.result().raw().whole_frames(),
                *step.result().raw().remainder_numerator(),
                *step.result().raw().frame_count(),
            )
        })
        .collect();
    assert_eq!(
        endpoints,
        [
            (2666, 2, 2666),
            (2666, 2, 0),
            (5333, 1, 2667),
            (8000, 0, 2667)
        ]
    );
    for index in 1..6 {
        let SpeechUtteranceIntentEvent::Segment(value) = &original.events()[index] else {
            panic!("segment")
        };
        let specification = value.prosody().duration().clone();
        let unresolved = intent(vec![segment(0), boundary(specification.clone())]);
        match prepare_intent_duration_grid(&unresolved, source.material(), &anchor, Some(&basis)) {
            Err(IntentDurationGridRefusal::Unresolved {
                event: 1,
                specification: actual,
            }) => assert_eq!(actual, specification),
            _ => panic!("unresolved boundary state must refuse at its global index"),
        }
    }
}
