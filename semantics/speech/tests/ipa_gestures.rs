#![cfg(feature = "semantic-bindings")]
//! Actual opaque allophone-choice composition gate; run in the Speech owner.

use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    allophone_selection::*, declared_context::ExplicitAllophoneContext, semantic::*,
};
#[allow(dead_code)]
mod fixture {

    include!("common/occurrence_intent.rs");
}
fn phone(id: &str, ipa: &str) -> SpeechPhone {
    SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new(id.into()).unwrap(),
        ipa.into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap()
}
fn inventory() -> SpeechInventory {
    let rule = SpeechPhonemeAllophone::new(
        BoundedSequence::new(),
        SpeechConfidence::new(conduit_core::IeeeF32::from_value(1.0)).unwrap(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unspecified(),
            StressSpecification::known(SpeechStress::Primary).unwrap(),
            SpeechSyllablePositionSpecification::known(SpeechSyllablePosition::Onset).unwrap(),
            SpeechPositionSpecification::unspecified(),
        )
        .unwrap(),
        PhoneId::new("phone/t-asp".into()).unwrap(),
        Some("demo/stressed-onset-aspiration/1".into()),
        SpeechRuleStatus::Productive,
    )
    .unwrap();
    let phoneme = SpeechPhoneme::new(
        BoundedSequence::new(),
        BoundedSequence::try_from_iter([rule]).unwrap(),
        Some(PhoneId::new("phone/t".into()).unwrap()),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeId::new("phoneme/t".into()).unwrap(),
        "t".into(),
        BoundedSequence::try_from_iter([
            PhoneId::new("phone/t".into()).unwrap(),
            PhoneId::new("phone/t-asp".into()).unwrap(),
        ])
        .unwrap(),
        SpeechSegmentStatus::Core,
    )
    .unwrap();
    SpeechInventory::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        BoundedSequence::try_from_iter([phoneme]).unwrap(),
        BoundedSequence::try_from_iter([phone("phone/t", "t"), phone("phone/t-asp", "tʰ")])
            .unwrap(),
    )
    .unwrap()
}
fn intent(stress: StressSpecification) -> SpeechUtteranceIntent {
    let SpeechUtteranceIntentEvent::Segment(base) = fixture::segment_stress(0, stress) else {
        unreachable!()
    };
    fixture::intent([SpeechUtteranceIntentEvent::segment(
        base.occurrence().clone(),
        PhoneSpecification::unspecified(),
        base.phoneme().clone(),
        base.prosody().clone(),
        base.provenance().clone(),
        base.sources().clone(),
        base.stress().clone(),
        base.word_position().clone(),
    )
    .unwrap()])
}
fn timing() -> SpeechGestureTiming {
    let anchor = conduit_audio::AudioTrajectoryAnchor::new(
        conduit_audio::AudioOriginIdentity::new(1).unwrap(),
        conduit_audio::AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let time = |n| conduit_audio::AudioTimeFraction::new(10, n).unwrap();
    SpeechGestureTiming::new(anchor, time(0), time(2), time(2), time(1)).unwrap()
}

use conduit_core::IeeeF32;
use conduit_speech::{ipa_inventory::*, ipa_shared::*, shared_intent::*};
fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "shared gesture test".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}
fn profile() -> SpeechIpaNotationProfile {
    SpeechIpaNotationProfile::new(
        BoundedSequence::new(),
        SpeechIpaNotationProfileId::new("test/1".into()).unwrap(),
        SpeechInventoryId::new("inventory".into()).unwrap(),
        provenance(),
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        BoundedSequence::try_from_iter([("t", "t"), ("t-asp", "tʰ")].map(|(id, spelling)| {
            SpeechIpaUnitDefinition::new(
                SpeechIpaUnitId::new(id.into()).unwrap(),
                SpeechIpaUnitKind::Segment,
                provenance(),
                SpeechIpaSpelling::new(spelling.into()).unwrap(),
            )
            .unwrap()
        }))
        .unwrap(),
        LanguageVariety::new(
            VarietyId::new("variety".into()).unwrap(),
            LanguageId::new("en".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn basis(id: &str) -> SpeechTokenSequenceBasis {
    SpeechTokenSequenceBasis::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        SpeechSegmentSequenceId::new(id.into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
fn occurrence(id: &str) -> LanguageSpeechTokenRef {
    let b = basis(id);
    LanguageSpeechTokenRef::new(
        b.inventory_id().clone(),
        b.language().clone(),
        0,
        b.revision_id().clone(),
        b.sequence_id().clone(),
        b.utterance_id().clone(),
    )
    .unwrap()
}
#[test]
fn joined_original_context_gestures_preserve_ipa_correspondence_and_syllable_custody() {
    let inventory = inventory();
    let notation = profile();
    let phones = [
        SpeechIpaPhoneDefinitionBinding::new(
            PhoneId::new("phone/t".into()).unwrap(),
            provenance(),
            BoundedSequence::try_from_iter([SpeechIpaUnitId::new("t".into()).unwrap()]).unwrap(),
        )
        .unwrap(),
        SpeechIpaPhoneDefinitionBinding::new(
            PhoneId::new("phone/t-asp".into()).unwrap(),
            provenance(),
            BoundedSequence::try_from_iter([SpeechIpaUnitId::new("t-asp".into()).unwrap()])
                .unwrap(),
        )
        .unwrap(),
    ];
    let phonemes = [SpeechIpaPhonemeDefinitionBinding::new(
        PhonemeId::new("phoneme/t".into()).unwrap(),
        provenance(),
        BoundedSequence::try_from_iter([SpeechIpaUnitId::new("t".into()).unwrap()]).unwrap(),
    )
    .unwrap()];
    let ipa = PreparedIpaInventory::prepare(
        &inventory,
        &notation,
        notation.variety(),
        notation.revision(),
        &phones,
        &phonemes,
    )
    .unwrap();
    let policy = SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false).unwrap();
    let position =
        SpeechSyllablePositionSpecification::known(SpeechSyllablePosition::Onset).unwrap();
    let prosody = SpeechProsodicContextSpecification::Unspecified;
    let style = SpeechCarefulStyleSpecification::known(false).unwrap();
    for stress in [SpeechStress::Primary, SpeechStress::Unstressed] {
        let i = intent(StressSpecification::known(stress).unwrap());
        let choice = select_intent_allophone(
            &i,
            0,
            &inventory,
            &policy,
            ExplicitAllophoneContext {
                syllable_position: &position,
                prosodic_context: &prosody,
                careful_style: &style,
            },
        )
        .unwrap();
        for (planned_position, phone_spec, valid) in [
            (
                SpeechSyllablePosition::Onset,
                PhoneSpecification::Unspecified,
                true,
            ),
            (
                SpeechSyllablePosition::Coda,
                PhoneSpecification::Unspecified,
                matches!(stress, SpeechStress::Unstressed),
            ),
            (
                SpeechSyllablePosition::Onset,
                PhoneSpecification::Unknown,
                false,
            ),
        ] {
            let qtoken = SpeechPhoneToken::new(
                BoundedSequence::new(),
                SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
                SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
                phone_spec,
                provenance(),
                None,
            )
            .unwrap();
            let ptoken = SpeechPhonemeToken::new(
                SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
                SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
                PhonemeSpecification::known(PhonemeId::new("phoneme/t".into()).unwrap()).unwrap(),
                provenance(),
                BoundedSequence::try_from_iter([qtoken.clone()]).unwrap(),
                None,
            )
            .unwrap();
            let p = SpeechPhonemeSequence::new(
                basis("phoneme-sequence"),
                BoundedSequence::try_from_iter([ptoken]).unwrap(),
            )
            .unwrap();
            let q = SpeechPhoneSequence::new(
                basis("sequence"),
                BoundedSequence::try_from_iter([qtoken]).unwrap(),
            )
            .unwrap();
            let correspondences = [SpeechPhonemePhoneCorrespondence::new(
                SpeechRealizationCorrespondenceKind::Realized,
                BoundedSequence::try_from_iter([occurrence("phoneme-sequence")]).unwrap(),
                BoundedSequence::try_from_iter([occurrence("sequence")]).unwrap(),
                provenance(),
            )
            .unwrap()];
            let syllables = [SpeechPlannedSyllableIntent::new(
                q.basis().clone(),
                SpeechSyllableId::new("syllable".into()).unwrap(),
                None,
                BoundedSequence::try_from_iter([planned_position]).unwrap(),
                BoundedSequence::try_from_iter([occurrence("sequence")]).unwrap(),
                provenance(),
                None,
                StressSpecification::known(stress).unwrap(),
            )
            .unwrap()];
            let context = SpeechUtteranceIntentContext::new(
                None,
                provenance(),
                SpeechReferenceProjectionCapability::new("retained_metadata_only".into()).unwrap(),
                SpeechSpeakerReferenceSpecification::Unknown,
                SpeechStyleReferenceSpecification::Unspecified,
                notation.variety().clone(),
            )
            .unwrap();
            let shared = PreparedSpeechUtteranceIntent::prepare(
                &i,
                SpeechIntentComponents {
                    context: &context,
                    intended_text: None,
                    phonemes: &p,
                    phones: &q,
                    morphemes: &[],
                    morpheme_texts: &[],
                    syllables: &syllables,
                    correspondences: &correspondences,
                },
            )
            .unwrap();
            let joined = PreparedIpaSpeechUtteranceIntent::prepare(&shared, &ipa).unwrap();

            let result = PreparedIpaContextualPhoneGestures::prepare(&joined, &choice, &timing());
            if !valid {
                assert!(matches!(result, Err(SharedGestureRefusal::Native(_))));
                continue;
            }
            let result = result.unwrap();
            assert!(core::ptr::eq(result.joined(), &joined));
            assert!(core::ptr::eq(result.contextual().choice(), &choice));
            assert!(core::ptr::eq(result.phone(), &q.tokens()[0]));
            assert_eq!(result.phoneme_links().len(), 1);
            assert_eq!(result.syllable_links().len(), 1);
            assert!(core::ptr::eq(
                result.phoneme_links()[0].membership().definition(),
                choice.phoneme()
            ));
            assert_eq!(
                result.contextual().lowered().declared_phone().ipa(),
                if matches!(stress, SpeechStress::Primary) {
                    "tʰ"
                } else {
                    "t"
                }
            );
            // Same opaque IPA/gesture carrier reaches the actual Source DSP.
            // This manually authored component does not grant text/play authority.
            use conduit_plot::rust_binding::NativeRustBinding;
            let grid = conduit_audio::AudioSampleRateBasis::new(
                timing().anchor().clone(),
                conduit_audio::AudioFrameQuantization::Floor,
                8000,
            )
            .unwrap();
            let cycle = conduit_audio::AudioCycleDuration::new(200, 1).unwrap();
            let renderer = conduit_speech::prepare_speech_gesture_renderer(
                result.contextual().lowered(),
                &grid.encode().unwrap(),
                &cycle.encode().unwrap(),
            )
            .unwrap();
            assert!(core::ptr::eq(
                renderer.original(),
                result.contextual().lowered()
            ));
            assert_eq!(renderer.frame_range(), 800..1600);
            let mut cursor = renderer.cursor();
            let mut frames = 0;
            let mut aspiration = 0;
            let mut nonzero = 0;
            while let Some(frame) = renderer.next(&mut cursor).unwrap() {
                frames += 1;
                aspiration += usize::from(*frame.gates().aspiration());
                nonzero += usize::from(frame.sample() != 0);
            }
            assert_eq!(frames, 800);
            assert_eq!(
                aspiration,
                if matches!(stress, SpeechStress::Primary) {
                    160
                } else {
                    0
                }
            );
            assert!(nonzero > 0);
            let foreign_inventory = inventory.clone();
            let foreign = select_intent_allophone(
                &i,
                0,
                &foreign_inventory,
                &policy,
                ExplicitAllophoneContext {
                    syllable_position: &position,
                    prosodic_context: &prosody,
                    careful_style: &style,
                },
            )
            .unwrap();
            assert!(matches!(
                PreparedIpaContextualPhoneGestures::prepare(&joined, &foreign, &timing()),
                Err(SharedGestureRefusal::ForeignInventory)
            ));
            let foreign_intent = i.clone();
            let foreign = select_intent_allophone(
                &foreign_intent,
                0,
                &inventory,
                &policy,
                ExplicitAllophoneContext {
                    syllable_position: &position,
                    prosodic_context: &prosody,
                    careful_style: &style,
                },
            )
            .unwrap();
            assert!(matches!(
                PreparedIpaContextualPhoneGestures::prepare(&joined, &foreign, &timing()),
                Err(SharedGestureRefusal::ForeignIntent)
            ));
        }
    }
}

use conduit_speech::ipa_gestures::*;
