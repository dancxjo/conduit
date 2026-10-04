//! Timing fixture. Phone/source resolution and commitment are separate work.
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    pronounce, semantic::*, utterance_timing::prepare_utterance_timing, Renderer, VoiceBoundary,
    VoiceEvent, MAXIMUM_EVENTS, SAMPLE_RATE_HZ,
};
pub fn write(output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut storage = [VoiceEvent::boundary(VoiceBoundary::word); MAXIMUM_EVENTS];
    let pronounced = pronounce("Hello, world!", &mut storage).map_err(|e| format!("{e:?}"))?;
    let inventory = SpeechInventoryId::new("timing/fixture".into()).unwrap();
    let language = SpeechLanguageId::new("en".into()).unwrap();
    let revision = SpeechSegmentRevisionId::new("timing/revision".into()).unwrap();
    let sequence = SpeechSegmentSequenceId::new("timing/phones".into()).unwrap();
    let utterance = SpeechUtteranceId::new("timing/hello".into()).unwrap();
    let provenance = SpeechEvidenceProvenance::new(
        "explicit timing listening fixture".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap();
    let sources = BoundedSequence::try_from_iter([LanguageSegmentRef::text(
        LanguageTextSegmentKind::Phrase,
        language.clone(),
        ListeningTextRange::new(13, 0).unwrap(),
        LanguageTextRevisionId::new("fixture/text-revision".into()).unwrap(),
        LanguageTextId::new("fixture/text".into()).unwrap(),
    )
    .unwrap()])
    .unwrap();
    let events = pronounced
        .events()
        .iter()
        .enumerate()
        .map(|(ordinal, event)| {
            let frames = Renderer::prepare(core::slice::from_ref(event))
                .unwrap()
                .total_frames();
            let duration =
                SpeechDurationSpecification::known(u64::from(SAMPLE_RATE_HZ), frames).unwrap();
            match event {
                VoiceEvent::boundary(boundary) => {
                    let kind = match boundary {
                        VoiceBoundary::word => SpeechBoundaryKind::Word,
                        VoiceBoundary::phrase => SpeechBoundaryKind::Phrase,
                        VoiceBoundary::turn => SpeechBoundaryKind::Turn,
                    };
                    SpeechUtteranceIntentEvent::boundary(
                        duration,
                        SpeechBoundarySpecification::known(kind).unwrap(),
                        provenance.clone(),
                        sources.clone(),
                    )
                    .unwrap()
                }
                _ => SpeechUtteranceIntentEvent::segment(
                    LanguageSpeechTokenRef::new(
                        inventory.clone(),
                        language.clone(),
                        ordinal as u32,
                        revision.clone(),
                        sequence.clone(),
                        utterance.clone(),
                    )
                    .unwrap(),
                    PhoneSpecification::unknown(),
                    PhonemeSpecification::unknown(),
                    SpeechSegmentProsodyIntent::new(
                        duration,
                        SpeechCycleSpecification::known(120, 1).unwrap(),
                        SpeechIntensitySpecification::known(1, 1).unwrap(),
                    )
                    .unwrap(),
                    provenance.clone(),
                    sources.clone(),
                    StressSpecification::unknown(),
                    SpeechPositionSpecification::unspecified(),
                )
                .unwrap(),
            }
        })
        .collect::<Vec<_>>();
    let intent = SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(events).unwrap(),
        inventory,
        language,
        provenance,
        revision,
        utterance,
    )
    .unwrap();
    let profile = SpeechFormantBoundaryProfile::new(
        BoundedSequence::try_from_iter([
            SpeechFormantBoundaryBinding::new(
                SpeechBoundaryKind::Word,
                SpeechFormantBoundary::Word,
            )
            .unwrap(),
            SpeechFormantBoundaryBinding::new(
                SpeechBoundaryKind::Phrase,
                SpeechFormantBoundary::Phrase,
            )
            .unwrap(),
            SpeechFormantBoundaryBinding::new(
                SpeechBoundaryKind::Turn,
                SpeechFormantBoundary::Turn,
            )
            .unwrap(),
        ])
        .unwrap(),
    )
    .unwrap();
    let prepared = prepare_utterance_timing(&intent, &profile).map_err(|e| format!("{e:?}"))?;
    // The caller's separately selected compact pronunciation is unchanged;
    // this timing receipt does not resolve the unknown phone specifications.
    let renderer = prepared
        .renderer(pronounced.events())
        .map_err(|e| format!("{e:?}"))?;
    super::write_rendered(output, "utterance-timing-hello-world", renderer)?;
    println!("{output}/utterance-timing-hello-world.wav: explicit 120 Hz/unity timing including exact pauses; no phone/source or commitment admission claim");
    Ok(())
}
