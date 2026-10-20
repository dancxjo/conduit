//! Actual frozen carrier traversal and Source-owned exact period preparation.
#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
use conduit_audio::*;
use conduit_plot::{rust_binding::NativeRustBinding, PortableExpressionProgram};
use conduit_speech::{
    carried_formant::FrozenCarriedFormant,
    complete_carrier::{CompleteCarrierAcoustics, PreparedCompleteSpeechCarrier},
    fargan_carrier_epochs::*,
    semantic::*,
};
fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
#[test]
fn source_period_preserves_original_ratio_remainder_and_half_up_without_clamping() {
    for (n, d, rounded) in [
        (1, 500, 32),
        (65, 32000, 33),
        (255, 16000, 255),
        (1, 120, 133),
    ] {
        let cycle = SpeechFundamentalCycle::new(d, n).unwrap();
        let prepared = prepare_fargan_period(&cycle).unwrap();
        assert_eq!(prepared.period(), rounded);
        assert_eq!(
            prepared.consumer_ledger(),
            PERIOD_CONSUMERS.map(|entry| (entry, rounded))
        );
        assert_eq!(prepared.projection().input().cycle(), &cycle);
        let program =
            PortableExpressionProgram::from_canonical_hex(prepared.source_program_hex()).unwrap();
        assert_eq!(
            program.evaluate(prepared.input_canonical()).unwrap(),
            prepared.output_canonical()
        );
        let raw = SpeechFarganPeriodRawProjection::decode(prepared.output_canonical()).unwrap();
        assert!(*raw.supported());
        let projected = prepared.projection();
        assert!(SpeechFarganPeriodProjection::new(
            projected.input().clone(),
            *projected.remainder_numerator(),
            *projected.rounded_frames() + 1,
            *projected.rounded_up(),
            *projected.whole_frames()
        )
        .is_err());
    }
    // Both unsupported exact ratios round into support; neither may be clamped/admitted.
    for (n, d) in [(127, 64000), (1021, 64000), (1, 1000), (1, 20)] {
        assert!(matches!(
            prepare_fargan_period(&SpeechFundamentalCycle::new(d, n).unwrap()),
            Err(CarrierEpochRefusal::UnsupportedCycle)
        ));
    }
    assert!(matches!(
        prepare_fargan_period(&SpeechFundamentalCycle::new(1, u64::MAX).unwrap()),
        Err(CarrierEpochRefusal::CycleBounds)
    ));
}
#[test]
fn complete_original_carrier_yields_actual_pcm_and_retains_every_blocked_epoch() {
    let f = fixture::Fixture::new();
    let inventory = f.inventory().unwrap();
    let original = f.correspondence(&inventory).unwrap();
    let evidence = f.complete_evidence();
    let structure = f.structure();
    let complete = PreparedCompleteSpeechCarrier::prepare(
        &original,
        &structure,
        &evidence.lexical,
        &f.word_phonemes,
        &evidence.request,
        &evidence.result,
        CompleteCarrierAcoustics::not_supplied(),
    )
    .unwrap();
    let basis = AudioSampleRateBasis::new(anchor(), AudioFrameQuantization::Floor, 8000).unwrap();
    let frozen =
        FrozenCarriedFormant::prepare(&complete, &f.formant, &f.boundaries, &anchor(), &basis)
            .unwrap();
    let before = complete.canonical_bytes().to_vec();
    let prepared = prepare_carrier_epochs(&complete, &frozen, 4096).unwrap();
    assert_eq!(prepared.source().original_bytes(), before);
    assert_eq!(prepared.source().source_identity(), &complete.identity());
    assert!(prepared
        .epochs()
        .iter()
        .flat_map(|e| e.pcm())
        .any(|v| *v != 0));
    let mut end = 0;
    let mut ready = 0;
    for epoch in prepared.epochs() {
        assert_eq!(epoch.start_frame(), end);
        assert!(epoch.pcm().len() <= 80 && !epoch.pcm().is_empty());
        end += epoch.pcm().len() as u64;
        if let Some(period) = epoch.period() {
            ready += 1;
            assert!(epoch.blockers().is_empty());
            assert_eq!(epoch.pcm().len(), 80);
            assert!((32..=255).contains(&period.period()));
            for &event in epoch.events() {
                let control = frozen.control_receipts()[event].0.as_ref().unwrap();
                assert_eq!(
                    control.request().cycle(),
                    period.projection().input().cycle()
                );
                assert_eq!(*control.request().sample_rate_hz(), 8000);
            }
        } else {
            assert!(!epoch.blockers().is_empty());
        }
    }
    assert_eq!(end, frozen.total_frames());
    assert!(
        ready > 0,
        "positive actual segment epochs must prepare, not only refusals"
    );
    assert!(matches!(
        prepare_carrier_epochs(&complete, &frozen, 0),
        Err(CarrierEpochRefusal::Capacity)
    ));
    assert_eq!(complete.canonical_bytes(), before);
    // Boundary/profile-mode intervals are retained as MissingCycle, never assigned
    // an arbitrary lag or removed to turn a partial result into whole-utterance proof.
    for (event, (span, (cycle, _))) in frozen
        .timing_receipts()
        .iter()
        .zip(frozen.control_receipts())
        .enumerate()
    {
        if cycle.is_none() && *span.frame_count() > 0 {
            assert!(prepared.epochs().iter().any(|epoch| epoch
                .blockers()
                .contains(&EpochBlocker::MissingCycle { event })));
        }
    }
}
