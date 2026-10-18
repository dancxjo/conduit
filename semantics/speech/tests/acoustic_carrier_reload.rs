//! Finite Q3 custody proof; no playback permission or trained voice claim.
#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
use conduit_audio::*;
use conduit_core::ExactNumericWork;
use conduit_language::*;
use conduit_plot::{rust_binding::NativeRustBinding, *};
use conduit_speech::{
    correspondence::PreparedSpeechCorrespondence, intent_duration_grid::*, intent_fundamental::*,
    ipa_inventory::PreparedIpaInventory, semantic::*,
};
use fixture::seq;
fn anchor(origin: u64) -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(origin).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn time(n: u64, d: u64) -> AudioExactTimeOffset {
    AudioExactTimeOffset::new(d, n).unwrap()
}
fn provenance() -> AudioTrajectoryProvenance {
    AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "Q3 checked literal source".into(),
        Some("1".into()),
    )
    .unwrap()
}
fn revised_basis(b: &SpeechTokenSequenceBasis) -> SpeechTokenSequenceBasis {
    SpeechTokenSequenceBasis::new(
        b.inventory_id().clone(),
        b.language().clone(),
        SpeechSegmentRevisionId::new("speech/acoustic-v1".into()).unwrap(),
        b.sequence_id().clone(),
        b.utterance_id().clone(),
    )
    .unwrap()
}
fn revised_ref(r: &LanguageSpeechTokenRef) -> LanguageSpeechTokenRef {
    LanguageSpeechTokenRef::new(
        r.inventory_id().clone(),
        r.language().clone(),
        *r.ordinal(),
        SpeechSegmentRevisionId::new("speech/acoustic-v1".into()).unwrap(),
        r.sequence_id().clone(),
        r.utterance_id().clone(),
    )
    .unwrap()
}
/// A new explicitly authored quantitative specimen with its own Speech revision.
/// The historical fixture function and its original values remain unchanged.
fn specimen() -> fixture::Fixture {
    let mut f = fixture::Fixture::new();
    f.phones = SpeechPhoneSequence::new(revised_basis(f.phones.basis()), f.phones.tokens().clone())
        .unwrap();
    f.phonemes = SpeechPhonemeSequence::new(
        revised_basis(f.phonemes.basis()),
        f.phonemes.tokens().clone(),
    )
    .unwrap();
    f.alignment = SpeechRealizationAlignment::new(
        seq((0..5).map(|n| {
            SpeechRealizationCorrespondence::aligned(
                seq([revised_ref(&fixture::occurrence("phonemes", n))]),
                seq([revised_ref(&fixture::occurrence("phones", n))]),
            )
            .unwrap()
        })),
        f.alignment.inventory_basis().clone(),
        f.phones.basis().clone(),
        f.phonemes.basis().clone(),
        fixture::evidence(),
        f.alignment.source().clone(),
    )
    .unwrap();
    f.intent = SpeechUtteranceIntent::new(
        seq(f
            .intent
            .events()
            .as_slice()
            .iter()
            .map(|event| match event {
                SpeechUtteranceIntentEvent::Segment(v) => SpeechUtteranceIntentEvent::segment(
                    revised_ref(v.occurrence()),
                    v.phone().clone(),
                    v.phoneme().clone(),
                    SpeechSegmentProsodyIntent::new(
                        SpeechDurationSpecification::known(3, 1).unwrap(),
                        SpeechCycleSpecification::known(
                            if *v.occurrence().ordinal() == 0 {
                                440
                            } else {
                                120
                            },
                            1,
                        )
                        .unwrap(),
                        v.prosody().relative_intensity().clone(),
                    )
                    .unwrap(),
                    v.provenance().clone(),
                    v.sources().clone(),
                    v.stress().clone(),
                    v.word_position().clone(),
                )
                .unwrap(),
                SpeechUtteranceIntentEvent::Boundary(v) => SpeechUtteranceIntentEvent::boundary(
                    SpeechDurationSpecification::known(3, 0).unwrap(),
                    v.kind().clone(),
                    v.provenance().clone(),
                    v.sources().clone(),
                )
                .unwrap(),
            })),
        f.intent.inventory_id().clone(),
        f.intent.language().clone(),
        fixture::evidence(),
        f.phones.basis().revision_id().clone(),
        f.intent.utterance_id().clone(),
    )
    .unwrap();
    f
}
fn literal_frequency(source: &str) -> Result<AudioFrequencyHz, ExactAudioControlRefusal> {
    let syntax = parse_syntax_document(source);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    assert_eq!(syntax.round_trip(), source);
    let CanonicalStartupValue::Quantity(value) = &checked.plots[0]
        .local_values
        .iter()
        .find(|(name, _)| name == "frequency")
        .unwrap()
        .1
    else {
        panic!("checked quantity")
    };
    let prepared = prepare_exact_audio_frequency(
        value.value(),
        value.source(),
        &mut ExactNumericWork::new(1_000_000_000),
    )?;
    assert_eq!(prepared.receipt().original(), value.source());
    assert_eq!(prepared.receipt().source(), value.value());
    Ok(prepared.value().clone())
}
fn segment(start: AudioExactTimeOffset, end: AudioExactTimeOffset) -> AudioTrajectorySegment {
    AudioTrajectorySegment::new(
        end,
        AudioTrajectoryInterpolation::Step,
        AudioTrajectoryQuantity::frequency(1, 440).unwrap(),
        AudioTrajectoryQuantity::frequency(1, 440).unwrap(),
        start,
    )
    .unwrap()
}
fn trajectory(segments: Vec<AudioTrajectorySegment>) -> AudioQuantityTrajectory {
    AudioQuantityTrajectory::new(
        anchor(1),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        provenance(),
        seq(segments),
    )
    .unwrap()
}
fn control_set(controls: Vec<AudioScopedControlTrajectory>) -> AudioControlTrajectorySet {
    AudioControlTrajectorySet::new(
        AudioControlConflictPolicy::RefuseOverlap,
        seq(controls),
        AudioControlGapPolicy::RefuseAnyGap,
    )
    .unwrap()
}
fn scoped(scope: &str, trajectory: AudioQuantityTrajectory) -> AudioScopedControlTrajectory {
    AudioScopedControlTrajectory::new(
        "F0".into(),
        AudioTrajectoryDomain::Frequency,
        0,
        scope.into(),
        trajectory,
    )
    .unwrap()
}

#[test]
fn checked_440hz_carrier_binding_and_trajectory_readmit_exactly_with_grid_remainders() {
    let authored = "plot acoustic {\n frequency = 440Hz\n}\n".to_string();
    let hz = literal_frequency(&authored).unwrap();
    assert_eq!(*hz.numerator_hz(), 440);
    assert!(literal_frequency("plot acoustic {\n frequency = 440m\n}\n").is_err());
    let f = specimen();
    let inventory = f.inventory().unwrap();
    let original = f.correspondence(&inventory).unwrap();
    let scope = SpeechPhoneAcousticScope::new(
        anchor(1),
        inventory.basis().clone(),
        revised_ref(&fixture::occurrence("phones", 0)),
        f.alignment.source().clone(),
    )
    .unwrap();
    let binding = SpeechFundamentalBinding::new(
        None,
        SpeechFundamentalTarget::new(
            AudioFrequencyControl::new(anchor(1), provenance(), hz).unwrap(),
            scope,
        )
        .unwrap(),
    )
    .unwrap();
    let prepared = prepare_phone_fundamental(&original, 0, &binding).unwrap();
    assert!(core::ptr::eq(prepared.original(), &original));
    let trajectory = trajectory(vec![segment(time(0, 1), time(1, 3))]);
    let source_bytes =
        conduit_speech::carrier_identity::canonical_carrier_bytes(&original).unwrap();
    // Separate exact component byte strings are test evidence, not a new public
    // artifact protocol or reconstruction of text/IPA from display output.
    let intent_bytes = f.intent.clone().encode().unwrap();
    let phone_bytes = f.phones.clone().encode().unwrap();
    let phoneme_bytes = f.phonemes.clone().encode().unwrap();
    let text_bytes = f.source.material().clone().encode().unwrap();
    let alignment_bytes = f.alignment.clone().encode().unwrap();
    let inventory_bytes = f.definitions.clone().encode().unwrap();
    let profile_bytes = f.notation_profile.clone().encode().unwrap();
    let phone_bindings: Vec<_> = f
        .phone_bindings
        .iter()
        .cloned()
        .map(|b| b.encode().unwrap())
        .collect();
    let phoneme_bindings: Vec<_> = f
        .phoneme_bindings
        .iter()
        .cloned()
        .map(|b| b.encode().unwrap())
        .collect();
    let binding_bytes = binding.clone().encode().unwrap();
    let trajectory_bytes = trajectory.clone().encode().unwrap();
    let reloaded_source = String::from_utf8(authored.clone().into_bytes()).unwrap();
    assert_eq!(
        literal_frequency(&reloaded_source).unwrap(),
        *binding.target().control().value()
    );
    let intent = SpeechUtteranceIntent::decode(&intent_bytes).unwrap();
    let phones = SpeechPhoneSequence::decode(&phone_bytes).unwrap();
    let phonemes = SpeechPhonemeSequence::decode(&phoneme_bytes).unwrap();
    let text = LanguageText::decode(&text_bytes).unwrap();
    let alignment = SpeechRealizationAlignment::decode(&alignment_bytes).unwrap();
    let definitions = SpeechInventory::decode(&inventory_bytes).unwrap();
    let profile = SpeechIpaNotationProfile::decode(&profile_bytes).unwrap();
    let phone_bindings: Vec<_> = phone_bindings
        .iter()
        .map(|b| SpeechIpaPhoneDefinitionBinding::decode(b).unwrap())
        .collect();
    let phoneme_bindings: Vec<_> = phoneme_bindings
        .iter()
        .map(|b| SpeechIpaPhonemeDefinitionBinding::decode(b).unwrap())
        .collect();
    let inventory = PreparedIpaInventory::prepare(
        &definitions,
        &profile,
        profile.variety(),
        profile.revision(),
        &phone_bindings,
        &phoneme_bindings,
    )
    .unwrap();
    let carrier = PreparedSpeechCorrespondence::prepare(
        &intent, &phonemes, &phones, &text, &alignment, &inventory,
    )
    .unwrap();
    assert_eq!(
        conduit_speech::carrier_identity::canonical_carrier_bytes(&carrier).unwrap(),
        source_bytes
    );
    let decoded_binding = SpeechFundamentalBinding::decode(&binding_bytes).unwrap();
    let reloaded = prepare_phone_fundamental(&carrier, 0, &decoded_binding).unwrap();
    assert_eq!(decoded_binding.clone().encode().unwrap(), binding_bytes);
    assert_eq!(
        reloaded.target().fundamental().reciprocal().result(),
        prepared.target().fundamental().reciprocal().result()
    );
    let decoded_trajectory = AudioQuantityTrajectory::decode(&trajectory_bytes).unwrap();
    assert_eq!(
        decoded_trajectory.clone().encode().unwrap(),
        trajectory_bytes
    );
    assert_eq!(
        decoded_trajectory.anchor(),
        binding.target().scope().anchor()
    );
    assert_eq!(
        decoded_binding.target().scope().inventory_basis().variety(),
        profile.variety()
    );
    let trajectory = PreparedAudioQuantityTrajectory::new(&trajectory_bytes).unwrap();
    let query = AudioTrajectoryQuery::new(anchor(1), time(1, 6))
        .unwrap()
        .encode()
        .unwrap();
    let evaluated = trajectory.evaluate(&query).unwrap();
    assert_eq!(
        evaluated.result(),
        &AudioTrajectoryQuantity::frequency(1, 440).unwrap()
    );
    for rate in [8000, 16000, 48000] {
        let basis =
            AudioSampleRateBasis::new(anchor(1), AudioFrameQuantization::Floor, rate).unwrap();
        let cycle = reloaded
            .target()
            .fundamental()
            .at_rate(Some(&basis))
            .unwrap();
        assert_eq!(*cycle.result().raw().remainder_numerator(), rate % 440);
        let before =
            prepare_intent_duration_grid(&f.intent, f.source.material(), &anchor(1), Some(&basis))
                .unwrap();
        let after = prepare_intent_duration_grid(&intent, &text, &anchor(1), Some(&basis)).unwrap();
        for (left, right) in before.steps().iter().zip(after.steps()) {
            assert_eq!(left.admitted_canonical(), right.admitted_canonical());
        }
        let last = after.steps().last().unwrap();
        assert_eq!(*last.result().raw().whole_frames(), 5 * rate / 3);
        assert_eq!(*last.result().raw().remainder_numerator(), 5 * rate % 3);
    }
    let foreign =
        AudioSampleRateBasis::new(anchor(2), AudioFrameQuantization::Floor, 8000).unwrap();
    assert!(reloaded
        .target()
        .fundamental()
        .at_rate(Some(&foreign))
        .is_err());
    let stale_text = LanguageText::new(
        text.identity().clone(),
        text.language().clone(),
        LanguageTextRevisionId::new("stale".into()).unwrap(),
        text.text().clone(),
    )
    .unwrap();
    assert!(PreparedSpeechCorrespondence::prepare(
        &intent,
        &phonemes,
        &phones,
        &stale_text,
        &alignment,
        &inventory
    )
    .is_err());
    let realized = conduit_speech::intent_realization::prepare_intent_realization(
        &intent,
        &definitions,
        &f.formant,
        &f.boundaries,
    )
    .unwrap();
    let mut renderer = realized.renderer().unwrap();
    assert!(renderer.render(&mut [0; 129]).is_err());
    assert_eq!(renderer.rendered_frames(), 0);
}

#[test]
fn real_generic_audio_controls_report_overlap_gap_and_foreign_anchor() {
    let one = trajectory(vec![segment(time(0, 1), time(1, 1))]);
    let controls = control_set(vec![
        scoped("scope/one", one.clone()),
        scoped("scope/two", one),
    ]);
    let prepared = PreparedAudioControlTrajectories::new(&controls.encode().unwrap()).unwrap();
    let query = AudioTrajectoryQuery::new(anchor(1), time(1, 2))
        .unwrap()
        .encode()
        .unwrap();
    assert!(matches!(
        prepared.evaluate(&query),
        Err(AudioControlRefusal::Conflict { .. })
    ));
    let gapped = trajectory(vec![
        segment(time(0, 1), time(1, 3)),
        segment(time(2, 3), time(1, 1)),
    ]);
    let controls = control_set(vec![scoped("scope/gapped", gapped)]);
    let prepared = PreparedAudioControlTrajectories::new(&controls.encode().unwrap()).unwrap();
    assert!(matches!(
        prepared.evaluate(&query),
        Err(AudioControlRefusal::Trajectory {
            refusal: AudioTrajectoryRefusal::MissingCoverage,
            ..
        })
    ));
    let foreign = AudioTrajectoryQuery::new(anchor(2), time(1, 6))
        .unwrap()
        .encode()
        .unwrap();
    assert!(matches!(
        prepared.evaluate(&foreign),
        Err(AudioControlRefusal::Trajectory {
            refusal: AudioTrajectoryRefusal::ForeignAnchor,
            ..
        })
    ));
}

#[test]
fn stale_commitment_basis_executes_source_refusal_without_advancing_frontier() {
    let original = fixture::commit_request();
    let accepted = fixture::execute_commitment(&original);
    assert_eq!(accepted.refusal(), &LanguageParserFrontierRefusal::None);
    assert_eq!(*accepted.frontier().committed(), 2);
    let basis = original.frontier().basis();
    let stale = LanguageParserBasis::new(
        basis.analysis_revision().clone(),
        LanguageTextRevisionId::new("stale/source".into()).unwrap(),
        basis.text().clone(),
    )
    .unwrap();
    let frontier = LanguageParserRawFrontier::new(stale, 0, 2).unwrap();
    let request = LanguageParserBeamCommitRequest::new(
        original.beam().clone(),
        false,
        frontier.clone(),
        false,
        2,
    )
    .unwrap();
    let refused = fixture::execute_commitment(&request);
    assert!(!refused.accepted());
    assert_eq!(
        refused.refusal(),
        &LanguageParserFrontierRefusal::WrongBasis
    );
    assert_eq!(refused.frontier(), &frontier);
    assert_eq!(*refused.candidate0().state().committed(), 0);
}
