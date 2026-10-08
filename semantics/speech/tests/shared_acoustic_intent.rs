#![cfg(feature = "semantic-bindings")]

use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{ipa_inventory::*, ipa_shared::*, semantic::*, shared_intent::*};
fn provenance(note: &str) -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(note.into(), SpeechEvidenceSource::Manual, None).unwrap()
}
fn profile(inventory: &str, variety: &str, revision: &str, note: &str) -> SpeechIpaNotationProfile {
    SpeechIpaNotationProfile::new(
        BoundedSequence::new(),
        SpeechIpaNotationProfileId::new("notation/test@1".into()).unwrap(),
        SpeechInventoryId::new(inventory.into()).unwrap(),
        provenance(note),
        SpeechSegmentRevisionId::new(revision.into()).unwrap(),
        BoundedSequence::try_from_iter([SpeechIpaUnitDefinition::new(
            SpeechIpaUnitId::new("schwa".into()).unwrap(),
            SpeechIpaUnitKind::Segment,
            provenance("unit evidence"),
            SpeechIpaSpelling::new("ə".into()).unwrap(),
        )
        .unwrap()])
        .unwrap(),
        LanguageVariety::new(
            VarietyId::new(variety.into()).unwrap(),
            LanguageId::new("language/test".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn inventory(ipa: &str) -> SpeechInventory {
    let phone = SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new("phone/distinct".into()).unwrap(),
        ipa.into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap();
    let phoneme = SpeechPhoneme::new(
        BoundedSequence::new(),
        BoundedSequence::new(),
        None,
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeId::new("phoneme/distinct".into()).unwrap(),
        "ə".into(),
        BoundedSequence::new(),
        SpeechSegmentStatus::Core,
    )
    .unwrap();
    SpeechInventory::new(
        SpeechInventoryId::new("inventory/a".into()).unwrap(),
        LanguageId::new("language/test".into()).unwrap(),
        BoundedSequence::try_from_iter([phoneme]).unwrap(),
        BoundedSequence::try_from_iter([phone]).unwrap(),
    )
    .unwrap()
}
fn bindings() -> (
    SpeechIpaPhoneDefinitionBinding,
    SpeechIpaPhonemeDefinitionBinding,
) {
    let units =
        || BoundedSequence::try_from_iter([SpeechIpaUnitId::new("schwa".into()).unwrap()]).unwrap();
    (
        SpeechIpaPhoneDefinitionBinding::new(
            PhoneId::new("phone/distinct".into()).unwrap(),
            provenance("phone notation"),
            units(),
        )
        .unwrap(),
        SpeechIpaPhonemeDefinitionBinding::new(
            PhonemeId::new("phoneme/distinct".into()).unwrap(),
            provenance("phoneme notation"),
            units(),
        )
        .unwrap(),
    )
}

fn sequence_basis(revision: &str) -> SpeechTokenSequenceBasis {
    SpeechTokenSequenceBasis::new(
        SpeechInventoryId::new("inventory/a".into()).unwrap(),
        LanguageId::new("language/test".into()).unwrap(),
        SpeechSegmentRevisionId::new(revision.into()).unwrap(),
        SpeechSegmentSequenceId::new("sequence".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
fn intent(revision: &str) -> SpeechUtteranceIntent {
    let b = sequence_basis(revision);
    SpeechUtteranceIntent::new(
        BoundedSequence::new(),
        b.inventory_id().clone(),
        b.language().clone(),
        provenance("original intent"),
        b.revision_id().clone(),
        b.utterance_id().clone(),
    )
    .unwrap()
}
fn context(variety: &str) -> SpeechUtteranceIntentContext {
    SpeechUtteranceIntentContext::new(
        None,
        provenance("context"),
        SpeechReferenceProjectionCapability::new("retained_metadata_only".into()).unwrap(),
        SpeechSpeakerReferenceSpecification::unknown(),
        SpeechStyleReferenceSpecification::unspecified(),
        LanguageVariety::new(
            VarietyId::new(variety.into()).unwrap(),
            LanguageId::new("language/test".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn shared<'a>(
    i: &'a SpeechUtteranceIntent,
    c: &'a SpeechUtteranceIntentContext,
    p: &'a SpeechPhonemeSequence,
    q: &'a SpeechPhoneSequence,
) -> PreparedSpeechUtteranceIntent<'a> {
    PreparedSpeechUtteranceIntent::prepare(
        i,
        SpeechIntentComponents {
            context: c,
            intended_text: None,
            phonemes: p,
            phones: q,
            morphemes: &[],
            morpheme_texts: &[],
            syllables: &[],
            correspondences: &[],
        },
    )
    .unwrap()
}
fn sequences(
    revision: &str,
    phone: PhoneSpecification,
    phoneme: PhonemeSpecification,
) -> (SpeechPhonemeSequence, SpeechPhoneSequence) {
    let confidence = SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap();
    let q = SpeechPhoneToken::new(
        BoundedSequence::new(),
        confidence,
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        phone,
        provenance("phone"),
        None,
    )
    .unwrap();
    let p = SpeechPhonemeToken::new(
        SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        phoneme,
        provenance("phoneme"),
        BoundedSequence::new(),
        None,
    )
    .unwrap();
    (
        SpeechPhonemeSequence::new(
            sequence_basis(revision),
            BoundedSequence::try_from_iter([p]).unwrap(),
        )
        .unwrap(),
        SpeechPhoneSequence::new(
            sequence_basis(revision),
            BoundedSequence::try_from_iter([q]).unwrap(),
        )
        .unwrap(),
    )
}

use conduit_audio::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::shared_acoustic::*;
fn anchor(id: u64) -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(id).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn scope(revision: &str) -> SpeechAcousticScope {
    let b = sequence_basis(revision);
    let source = LanguageSegmentRef::phone(
        b.inventory_id().clone(),
        b.language().clone(),
        0,
        b.revision_id().clone(),
        b.sequence_id().clone(),
        b.utterance_id().clone(),
    )
    .unwrap();
    SpeechAcousticScope::new(
        "explicit-segment".into(),
        SpeechAcousticScopeKind::Segment,
        BoundedSequence::try_from_iter([source]).unwrap(),
    )
    .unwrap()
}
fn trajectory(hz: u64) -> AudioQuantityTrajectory {
    let v = AudioTrajectoryQuantity::frequency(1, hz).unwrap();
    AudioQuantityTrajectory::new(
        anchor(1),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        AudioTrajectoryProvenance::new(
            AudioTrajectoryProvenanceKind::Authored,
            "exact step".into(),
            Some("v1".into()),
        )
        .unwrap(),
        BoundedSequence::try_from_iter([AudioTrajectorySegment::new(
            AudioExactTimeOffset::new(1, 1).unwrap(),
            AudioTrajectoryInterpolation::Step,
            v.clone(),
            v,
            AudioExactTimeOffset::new(1, 0).unwrap(),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn original_join_and_every_candidate_survive_sample_time_evaluation() {
    let inv = inventory("ə");
    let notation = profile("inventory/a", "variety/a", "revision/a", "original");
    let (pb, qb) = bindings();
    let ipa = PreparedIpaInventory::prepare(
        &inv,
        &notation,
        notation.variety(),
        notation.revision(),
        core::slice::from_ref(&pb),
        core::slice::from_ref(&qb),
    )
    .unwrap();
    let i = intent("revision/a");
    let c = context("variety/a");
    let (p, q) = sequences(
        "revision/a",
        PhoneSpecification::Unknown,
        PhonemeSpecification::Unknown,
    );
    let shared = shared(&i, &c, &p, &q);
    let joined = PreparedIpaSpeechUtteranceIntent::prepare(&shared, &ipa).unwrap();
    let a = anchor(1);
    let first = trajectory(1500);
    let second = trajectory(2500);
    let states = [
        (known_audio_spec(first.clone()), 1),
        (
            SpeechAudioTrajectorySpecification::variable(
                BoundedSequence::try_from_iter([first.clone(), second.clone()]).unwrap(),
            )
            .unwrap(),
            2,
        ),
        (
            SpeechAudioTrajectorySpecification::gradient(
                SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
                second,
            )
            .unwrap(),
            1,
        ),
        (SpeechAudioTrajectorySpecification::Unknown, 0),
        (SpeechAudioTrajectorySpecification::Unspecified, 0),
        (SpeechAudioTrajectorySpecification::NotApplicable, 0),
    ];
    for (spec, count) in states {
        let targets = [SpeechTimingPitchTargets::new(
            a.clone(),
            SpeechDurationSpecification::Unknown,
            SpeechDurationCurveSpecification::Unspecified,
            spec.clone(),
            provenance("target"),
            scope("revision/a"),
        )
        .unwrap()];
        let components = SpeechAcousticComponents {
            timing_pitch: &targets,
            linguistic_rate: &[],
            intensity: &[],
            formants: &[],
            voice_quality: &[],
            observations: &[],
        };
        let prepared = PreparedSharedAcousticIntent::prepare(&joined, &a, components).unwrap();
        assert!(core::ptr::eq(prepared.joined(), &joined));
        assert!(core::ptr::eq(
            prepared.components().timing_pitch,
            &targets[..]
        ));
        assert_eq!(prepared.components().timing_pitch[0].pitch(), &spec);
        assert_eq!(prepared.candidates().len(), count);
        assert_eq!(prepared.scopes().len(), 1);
        let AcousticResolvedSource::Phone(resolved) = &prepared.scopes()[0].resolved()[0] else {
            panic!("phone")
        };
        assert!(core::ptr::eq(resolved.snapshot(), &q));
        for candidate in prepared.candidates() {
            let AcousticFieldPreparation::Audio(audio) = &candidate.preparation else {
                panic!("audio")
            };
            let query =
                AudioTrajectoryQuery::new(a.clone(), AudioExactTimeOffset::new(8000, 1).unwrap())
                    .unwrap();
            let receipt = audio.query(&query.encode().unwrap()).unwrap();
            assert!(matches!(
                receipt.audio().result(),
                AudioTrajectoryQuantity::Frequency(_)
            ));
        }
    }
    for (target_anchor, revision, expected_anchor) in [
        (anchor(2), "revision/a", true),
        (a.clone(), "foreign", false),
    ] {
        let targets = [SpeechTimingPitchTargets::new(
            target_anchor,
            SpeechDurationSpecification::Unknown,
            SpeechDurationCurveSpecification::Unknown,
            SpeechAudioTrajectorySpecification::Unknown,
            provenance("invalid"),
            scope(revision),
        )
        .unwrap()];
        let components = SpeechAcousticComponents {
            timing_pitch: &targets,
            linguistic_rate: &[],
            intensity: &[],
            formants: &[],
            voice_quality: &[],
            observations: &[],
        };
        let refusal = PreparedSharedAcousticIntent::prepare(&joined, &a, components)
            .err()
            .unwrap();
        if expected_anchor {
            assert!(matches!(refusal, SharedAcousticRefusal::ForeignAnchor));
        } else {
            assert!(matches!(refusal, SharedAcousticRefusal::MissingSource));
        }
    }
}

fn probability_curve() -> SpeechProbabilityCurve {
    SpeechProbabilityCurve::new(
        anchor(1),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        provenance("probability"),
        BoundedSequence::try_from_iter([SpeechProbabilityCurveSegment::new(
            AudioTimeFraction::new(1, 1).unwrap(),
            AudioTrajectoryInterpolation::Step,
            AudioTimeFraction::new(1, 0).unwrap(),
            SpeechProbabilitySpecification::variable(
                BoundedSequence::try_from_iter([
                    SpeechUnitInterval::new(4, 1).unwrap(),
                    SpeechUnitInterval::new(4, 3).unwrap(),
                ])
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
fn rate_curve(revision: &str) -> SpeechSyllabicRateCurve {
    SpeechSyllabicRateCurve::new(
        anchor(1),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        provenance("rate"),
        BoundedSequence::try_from_iter([SpeechSyllabicRateCurveSegment::new(
            AudioTimeFraction::new(1, 1).unwrap(),
            AudioTrajectoryInterpolation::Step,
            AudioTimeFraction::new(1, 0).unwrap(),
            SpeechSyllabicRateSpecification::known(1, 3, scope(revision)).unwrap(),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn all_target_ports_compose_and_nested_rate_scope_cannot_escape_original_material() {
    let inv = inventory("ə");
    let notation = profile("inventory/a", "variety/a", "revision/a", "original");
    let (pb, qb) = bindings();
    let ipa = PreparedIpaInventory::prepare(
        &inv,
        &notation,
        notation.variety(),
        notation.revision(),
        core::slice::from_ref(&pb),
        core::slice::from_ref(&qb),
    )
    .unwrap();
    let i = intent("revision/a");
    let c = context("variety/a");
    let (p, q) = sequences(
        "revision/a",
        PhoneSpecification::Unknown,
        PhonemeSpecification::Unknown,
    );
    let shared = shared(&i, &c, &p, &q);
    let joined = PreparedIpaSpeechUtteranceIntent::prepare(&shared, &ipa).unwrap();
    let a = anchor(1);
    let timing = [SpeechTimingPitchTargets::new(
        a.clone(),
        SpeechDurationSpecification::known(2, 1).unwrap(),
        SpeechDurationCurveSpecification::Unknown,
        SpeechAudioTrajectorySpecification::Unknown,
        provenance("timing"),
        scope("revision/a"),
    )
    .unwrap()];
    let intensity = [SpeechIntensityTargets::new(
        SpeechAudioTrajectorySpecification::Unknown,
        a.clone(),
        SpeechDecibelCurveSpecification::NotApplicable,
        provenance("intensity"),
        scope("revision/a"),
    )
    .unwrap()];
    let formants = [SpeechFormantTargets::new(
        a.clone(),
        BoundedSequence::try_from_iter([SpeechFormantTarget::new(
            known_audio_spec(trajectory(100)),
            known_audio_spec(trajectory(1500)),
            2,
            provenance("formant"),
        )
        .unwrap()])
        .unwrap(),
        provenance("formants"),
        scope("revision/a"),
    )
    .unwrap()];
    let voice = [SpeechVoiceQualityTargets::new(
        a.clone(),
        SpeechProbabilityCurveSpecification::Unknown,
        provenance("voice"),
        scope("revision/a"),
        SpeechSpectralTiltCurveSpecification::Unspecified,
        known_probability_spec(probability_curve()),
    )
    .unwrap()];
    for (revision, valid) in [("revision/a", true), ("foreign", false)] {
        let rate = [SpeechLinguisticRateTargets::new(
            a.clone(),
            provenance("rate"),
            scope("revision/a"),
            known_rate_spec(rate_curve(revision)),
        )
        .unwrap()];
        let components = SpeechAcousticComponents {
            timing_pitch: &timing,
            linguistic_rate: &rate,
            intensity: &intensity,
            formants: &formants,
            voice_quality: &voice,
            observations: &[],
        };
        let result = PreparedSharedAcousticIntent::prepare(&joined, &a, components);
        if !valid {
            assert!(matches!(result, Err(SharedAcousticRefusal::MissingSource)));
            continue;
        }
        let prepared = result.unwrap();
        assert_eq!(prepared.candidates().len(), 5);
        assert_eq!(prepared.scopes().len(), 6);
        let formant_fields: Vec<_> = prepared
            .candidates()
            .iter()
            .filter(|v| v.component == "formants")
            .collect();
        assert_eq!(formant_fields.len(), 2);
        assert!(formant_fields.iter().all(|v| v.formant_index == Some(2)));
        assert_ne!(formant_fields[0].field, formant_fields[1].field);
        let probability = prepared
            .candidates()
            .iter()
            .find(|v| v.field == "voicing_probability")
            .unwrap();
        let AcousticFieldPreparation::Probability(curve) = &probability.preparation else {
            panic!("probability")
        };
        let query =
            AudioTrajectoryQuery::new(a.clone(), AudioExactTimeOffset::new(8000, 1).unwrap())
                .unwrap();
        let receipt = curve.query(&query.encode().unwrap()).unwrap();
        let SpeechProbabilitySpecification::Variable(values) =
            SpeechProbabilitySpecification::decode(receipt.result_canonical()).unwrap()
        else {
            panic!("all candidates")
        };
        assert_eq!(values.as_slice().len(), 2);
    }
}

fn observation(a: AudioTrajectoryAnchor) -> SpeechCommonAcousticEvidence {
    SpeechCommonAcousticEvidence::new(
        SpeechFrameConfidenceSpecification::Unknown,
        SpeechDecibelLevelSpecification::Unknown,
        SpeechFrequencySpecification::NotApplicable,
        BoundedSequence::new(),
        SpeechHarmonicitySpecification::Unknown,
        SpeechProbabilitySpecification::known(4, 3).unwrap(),
        provenance("observed"),
        None,
        SpeechCommonAcousticSpan::new(
            a,
            AudioTimeFraction::new(1, 1).unwrap(),
            AudioTimeFraction::new(1, 0).unwrap(),
        )
        .unwrap(),
        SpeechSpectralCentroidSpecification::Unknown,
        SpeechSpectralTiltSpecification::Unknown,
        BoundedSequence::new(),
        BoundedSequence::new(),
        SpeechProbabilitySpecification::Unknown,
        SpeechZeroCrossingSpecification::Unknown,
    )
    .unwrap()
}
#[test]
fn text_and_original_event_provenance_are_borrowed_and_observations_stay_separate() {
    let inv = inventory("ə");
    let notation = profile("inventory/a", "variety/a", "revision/a", "original");
    let (pb, qb) = bindings();
    let ipa = PreparedIpaInventory::prepare(
        &inv,
        &notation,
        notation.variety(),
        notation.revision(),
        core::slice::from_ref(&pb),
        core::slice::from_ref(&qb),
    )
    .unwrap();
    let recognition = LanguageSegmentRef::recognition(
        ListeningEventId::new("event".into()).unwrap(),
        ListeningStreamId::new("stream".into()).unwrap(),
    )
    .unwrap();
    let original = intent("revision/a");
    let i = SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter([SpeechUtteranceIntentEvent::boundary(
            SpeechDurationSpecification::Unknown,
            SpeechBoundarySpecification::Unspecified,
            provenance("original source"),
            BoundedSequence::try_from_iter([recognition.clone()]).unwrap(),
        )
        .unwrap()])
        .unwrap(),
        original.inventory_id().clone(),
        original.language().clone(),
        original.provenance().clone(),
        original.revision_id().clone(),
        original.utterance_id().clone(),
    )
    .unwrap();
    let text = LanguageText::new(
        LanguageTextId::new("original-text".into()).unwrap(),
        LanguageId::new("language/test".into()).unwrap(),
        LanguageTextRevisionId::new("text/a".into()).unwrap(),
        "Olá".into(),
    )
    .unwrap();
    let text_reference = LanguageTextSegmentRef::new(
        LanguageTextSegmentKind::Utterance,
        text.language().clone(),
        LanguageTextRange::new(3, 0).unwrap(),
        text.revision().clone(),
        text.identity().clone(),
    )
    .unwrap();
    let base = context("variety/a");
    let c = SpeechUtteranceIntentContext::new(
        Some(text_reference.clone()),
        base.provenance().clone(),
        base.reference_capability().clone(),
        base.speaker().clone(),
        base.style().clone(),
        base.variety().clone(),
    )
    .unwrap();
    let (p, q) = sequences(
        "revision/a",
        PhoneSpecification::Unknown,
        PhonemeSpecification::Unknown,
    );
    let shared = PreparedSpeechUtteranceIntent::prepare(
        &i,
        SpeechIntentComponents {
            context: &c,
            intended_text: Some(&text),
            phonemes: &p,
            phones: &q,
            morphemes: &[],
            morpheme_texts: &[],
            syllables: &[],
            correspondences: &[],
        },
    )
    .unwrap();
    let joined = PreparedIpaSpeechUtteranceIntent::prepare(&shared, &ipa).unwrap();
    let a = anchor(1);
    let text_source = LanguageSegmentRef::text(
        *text_reference.kind(),
        text_reference.language().clone(),
        text_reference.range().clone(),
        text_reference.revision_id().clone(),
        text_reference.text_id().clone(),
    )
    .unwrap();
    let original_scope = SpeechAcousticScope::new(
        "retained-sources".into(),
        SpeechAcousticScopeKind::Utterance,
        BoundedSequence::try_from_iter([text_source, recognition]).unwrap(),
    )
    .unwrap();
    let target = SpeechTimingPitchTargets::new(
        a.clone(),
        SpeechDurationSpecification::Unknown,
        SpeechDurationCurveSpecification::Unknown,
        SpeechAudioTrajectorySpecification::Unknown,
        provenance("target"),
        original_scope,
    )
    .unwrap();
    let targets = [target.clone()];
    let observations = [observation(a.clone())];
    let prepared = PreparedSharedAcousticIntent::prepare(
        &joined,
        &a,
        SpeechAcousticComponents {
            timing_pitch: &targets,
            linguistic_rate: &[],
            intensity: &[],
            formants: &[],
            voice_quality: &[],
            observations: &observations,
        },
    )
    .unwrap();
    let sources = prepared.scopes()[0].resolved();
    let AcousticResolvedSource::Text(resolved) = &sources[0] else {
        panic!("text")
    };
    assert!(core::ptr::eq(resolved.material(), &text));
    assert_eq!(resolved.text(), "Olá");
    let AcousticResolvedSource::OriginalEvent { event, witness } = &sources[1] else {
        panic!("event")
    };
    assert!(core::ptr::eq(*event, &i.events()[0]));
    assert_eq!(witness.event(), *event);
    assert_eq!(prepared.observations()[0].original(), &observations[0]);
    assert!(prepared.observations()[0]
        .require_interpreted_measurements()
        .is_err());
    let foreign = [observation(anchor(2))];
    assert!(matches!(
        PreparedSharedAcousticIntent::prepare(
            &joined,
            &a,
            SpeechAcousticComponents {
                timing_pitch: &[],
                linguistic_rate: &[],
                intensity: &[],
                formants: &[],
                voice_quality: &[],
                observations: &foreign
            }
        ),
        Err(SharedAcousticRefusal::ForeignAnchor)
    ));
    let excessive = vec![target; 33];
    assert!(matches!(
        PreparedSharedAcousticIntent::prepare(
            &joined,
            &a,
            SpeechAcousticComponents {
                timing_pitch: &excessive,
                linguistic_rate: &[],
                intensity: &[],
                formants: &[],
                voice_quality: &[],
                observations: &[]
            }
        ),
        Err(SharedAcousticRefusal::Native(_))
    ));
}

fn known_audio_spec(v: AudioQuantityTrajectory) -> SpeechAudioTrajectorySpecification {
    SpeechAudioTrajectorySpecification::known(
        v.anchor().clone(),
        *v.endpoints(),
        *v.outside(),
        v.provenance().clone(),
        v.segments().clone(),
    )
    .unwrap()
}
fn known_probability_spec(v: SpeechProbabilityCurve) -> SpeechProbabilityCurveSpecification {
    SpeechProbabilityCurveSpecification::known(
        v.anchor().clone(),
        *v.endpoints(),
        *v.outside(),
        v.provenance().clone(),
        v.segments().clone(),
    )
    .unwrap()
}
fn known_rate_spec(v: SpeechSyllabicRateCurve) -> SpeechRateCurveSpecification {
    SpeechRateCurveSpecification::known(
        v.anchor().clone(),
        *v.endpoints(),
        *v.outside(),
        v.provenance().clone(),
        v.segments().clone(),
    )
    .unwrap()
}
