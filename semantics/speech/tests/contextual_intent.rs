#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    contextual_intent_realization::*, declared_context::ExplicitAllophoneContext, semantic::*,
    EnglishPhone, EnglishStress, Renderer, SpeechPhoneInput, VoiceBoundary, VoiceEvent,
};
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod occurrence;
#[allow(dead_code)]
#[path = "common/native_phone.rs"]
mod phone;
fn segment(
    ordinal: u32,
    position: SpeechPositionSpecification,
    duration: SpeechDurationSpecification,
) -> SpeechUtteranceIntentEvent {
    let SpeechUtteranceIntentEvent::Segment(original) = occurrence::segment(ordinal) else {
        panic!()
    };
    SpeechUtteranceIntentEvent::segment(
        original.occurrence().clone(),
        PhoneSpecification::unspecified(),
        original.phoneme().clone(),
        SpeechSegmentProsodyIntent::new(
            duration,
            SpeechCycleSpecification::known(120, 1).unwrap(),
            SpeechIntensitySpecification::known(1, 1).unwrap(),
        )
        .unwrap(),
        original.provenance().clone(),
        original.sources().clone(),
        original.stress().clone(),
        position,
    )
    .unwrap()
}
fn boundary() -> SpeechUtteranceIntentEvent {
    let SpeechUtteranceIntentEvent::Boundary(original) =
        occurrence::boundary(SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap())
    else {
        panic!()
    };
    SpeechUtteranceIntentEvent::boundary(
        SpeechDurationSpecification::known(30, 1).unwrap(),
        original.kind().clone(),
        original.provenance().clone(),
        original.sources().clone(),
    )
    .unwrap()
}
fn inventory() -> SpeechInventory {
    let rule = SpeechPhonemeAllophone::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unspecified(),
            StressSpecification::unspecified(),
            SpeechSyllablePositionSpecification::unspecified(),
            SpeechPositionSpecification::known(SpeechWordPosition::Initial).unwrap(),
        )
        .unwrap(),
        phone::id("phone/aspirated-t"),
        None,
        SpeechRuleStatus::Productive,
    )
    .unwrap();
    SpeechInventory::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        BoundedSequence::try_from_iter([SpeechPhoneme::new(
            BoundedSequence::new(),
            BoundedSequence::try_from_iter([rule]).unwrap(),
            Some(phone::id("phone/t")),
            SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
            PhonemeId::new("phoneme/t".into()).unwrap(),
            "t".into(),
            BoundedSequence::new(),
            SpeechSegmentStatus::Core,
        )
        .unwrap()])
        .unwrap(),
        BoundedSequence::try_from_iter([
            phone::definition("phone/t"),
            SpeechPhone::new(
                BoundedSequence::new(),
                SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
                phone::id("phone/aspirated-t"),
                "tʰ".into(),
                SpeechSegmentStatus::Allophonic,
            )
            .unwrap(),
        ])
        .unwrap(),
    )
    .unwrap()
}
fn voice(inventory: &SpeechInventory) -> SpeechFormantVoiceProfile {
    SpeechFormantVoiceProfile::new(
        "contextual fixture".into(),
        inventory.identity().clone(),
        inventory.language().clone(),
        BoundedSequence::try_from_iter(
            inventory
                .phones()
                .as_slice()
                .iter()
                .cloned()
                .zip([
                    conduit_speech::semantic::EnglishPhone::T,
                    conduit_speech::semantic::EnglishPhone::TAspirated,
                ])
                .map(|(definition, realization)| {
                    SpeechFormantPhoneBinding::new(definition, realization).unwrap()
                }),
        )
        .unwrap(),
    )
    .unwrap()
}
struct Context {
    syllable: SpeechSyllablePositionSpecification,
    prosody: SpeechProsodicContextSpecification,
    style: SpeechCarefulStyleSpecification,
}
impl Context {
    fn new() -> Self {
        Self {
            syllable: SpeechSyllablePositionSpecification::unknown(),
            prosody: SpeechProsodicContextSpecification::unknown(),
            style: SpeechCarefulStyleSpecification::known(false).unwrap(),
        }
    }
    fn get(&self) -> Option<ExplicitAllophoneContext<'_>> {
        Some(ExplicitAllophoneContext {
            syllable_position: &self.syllable,
            prosodic_context: &self.prosody,
            careful_style: &self.style,
        })
    }
}
fn boundaries() -> SpeechFormantBoundaryProfile {
    SpeechFormantBoundaryProfile::new(
        BoundedSequence::try_from_iter([SpeechFormantBoundaryBinding::new(
            SpeechBoundaryKind::Word,
            SpeechFormantBoundary::Word,
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
fn policy(default: bool, productive: bool) -> SpeechAllophoneChoicePolicy {
    SpeechAllophoneChoicePolicy::new(default, false, false, false, productive, false).unwrap()
}
fn pcm(mut renderer: Renderer<'_>, block: usize) -> Vec<i16> {
    let mut result = Vec::new();
    let mut buffer = [0; 128];
    while !renderer.is_complete() {
        let count = renderer.render(&mut buffer[..block]).unwrap();
        result.extend_from_slice(&buffer[..count]);
    }
    result
}
#[test]
fn choice_timing_and_silent_boundary_keep_original_identity_and_chunk_invariance() {
    let source = occurrence::intent([
        segment(
            10,
            SpeechPositionSpecification::known(SpeechWordPosition::Initial).unwrap(),
            SpeechDurationSpecification::known(30, 1).unwrap(),
        ),
        boundary(),
        segment(
            11,
            SpeechPositionSpecification::known(SpeechWordPosition::Medial).unwrap(),
            SpeechDurationSpecification::known(30, 1).unwrap(),
        ),
    ]);
    let inventory = inventory();
    let voice = voice(&inventory);
    let boundaries = boundaries();
    let context = Context::new();
    let policy = policy(true, true);
    let prepared = prepare_contextual_intent(
        &source,
        &inventory,
        &voice,
        &boundaries,
        &policy,
        &[context.get(), None, context.get()],
    )
    .unwrap();
    assert!(core::ptr::eq(prepared.source(), &source));
    assert!(core::ptr::eq(prepared.profile(), &voice));
    assert_eq!(prepared.phones().len(), 2);
    for (receipt, event) in prepared.phones().iter().zip([0, 2]) {
        let SpeechUtteranceIntentEvent::Segment(original) = &source.events().as_slice()[event]
        else {
            panic!()
        };
        assert!(core::ptr::eq(
            receipt.choice().occurrence().segment(),
            original
        ));
        assert!(matches!(original.phone(), PhoneSpecification::Unspecified));
        assert_eq!(
            receipt.choice().selected_phone(),
            Some(receipt.definition().identity())
        );
        assert_eq!(receipt.binding().definition(), receipt.definition());
    }
    let expected = [
        VoiceEvent::phone(SpeechPhoneInput {
            phone: EnglishPhone::t_aspirated,
            stress: EnglishStress::primary,
        }),
        VoiceEvent::boundary(VoiceBoundary::word),
        VoiceEvent::phone(SpeechPhoneInput {
            phone: EnglishPhone::t,
            stress: EnglishStress::primary,
        }),
    ];
    assert_eq!(prepared.events(), expected);
    let reference = pcm(prepared.timing().renderer(&expected).unwrap(), 128);
    assert_eq!(pcm(prepared.renderer().unwrap(), 1), reference);
    assert_eq!(pcm(prepared.renderer().unwrap(), 63), reference);
    assert_eq!(reference.len(), 800);
    assert!(reference[266..533].iter().all(|sample| *sample == 0));
    assert_eq!(
        prepared.phones()[0].choice().state().outcome(),
        &SpeechAllophoneChoiceOutcome::SelectedAllophone
    );
    assert_eq!(
        prepared.phones()[1].choice().state().outcome(),
        &SpeechAllophoneChoiceOutcome::SelectedDefault
    );
}
#[test]
fn missing_or_misaligned_context_refuses_before_quantitative_preparation() {
    let source = occurrence::intent([
        segment(
            0,
            SpeechPositionSpecification::unknown(),
            SpeechDurationSpecification::unknown(),
        ),
        boundary(),
    ]);
    let inventory = inventory();
    let voice = voice(&inventory);
    let boundaries = boundaries();
    let context = Context::new();
    let policy = policy(true, true);
    assert!(matches!(
        prepare_contextual_intent(&source, &inventory, &voice, &boundaries, &policy, &[]),
        Err(ContextualIntentRefusal::ContextCount)
    ));
    assert!(matches!(
        prepare_contextual_intent(
            &source,
            &inventory,
            &voice,
            &boundaries,
            &policy,
            &[None, None]
        ),
        Err(ContextualIntentRefusal::MissingContext { event: 0 })
    ));
    assert!(matches!(
        prepare_contextual_intent(
            &source,
            &inventory,
            &voice,
            &boundaries,
            &policy,
            &[context.get(), context.get()]
        ),
        Err(ContextualIntentRefusal::BoundaryContext { event: 1 })
    ));
    assert!(matches!(
        prepare_contextual_intent(
            &source,
            &inventory,
            &voice,
            &boundaries,
            &policy,
            &[context.get(), None]
        ),
        Err(ContextualIntentRefusal::Timing(_))
    ));
}
#[test]
fn deferred_or_missing_late_choice_returns_no_partially_playable_utterance() {
    let inventory = inventory();
    let voice = voice(&inventory);
    let boundaries = boundaries();
    let context = Context::new();
    for (position, policy) in [
        (SpeechPositionSpecification::unknown(), policy(true, true)),
        (
            SpeechPositionSpecification::known(SpeechWordPosition::Medial).unwrap(),
            policy(false, true),
        ),
    ] {
        let source = occurrence::intent([
            segment(
                0,
                SpeechPositionSpecification::known(SpeechWordPosition::Initial).unwrap(),
                SpeechDurationSpecification::known(30, 1).unwrap(),
            ),
            segment(
                1,
                position,
                SpeechDurationSpecification::known(30, 1).unwrap(),
            ),
        ]);
        let Err(ContextualIntentRefusal::Unchosen { event, state }) = prepare_contextual_intent(
            &source,
            &inventory,
            &voice,
            &boundaries,
            &policy,
            &[context.get(), context.get()],
        ) else {
            panic!("expected an exact unchosen receipt")
        };
        assert_eq!(event, 1);
        if *policy.allow_default() {
            assert_eq!(state.outcome(), &SpeechAllophoneChoiceOutcome::Deferred);
            assert_eq!(
                state.reason(),
                &SpeechContextDecision::ObservationUnresolved
            );
        } else {
            assert_eq!(state.outcome(), &SpeechAllophoneChoiceOutcome::None);
        }
    }
}

fn source_text(revision: &str) -> LanguageText {
    LanguageText::new(
        LanguageTextId::new("source".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        LanguageTextRevisionId::new(revision.into()).unwrap(),
        "t".into(),
    )
    .unwrap()
}
#[test]
fn sourced_choice_keeps_exact_materials_and_original_occurrences_through_pcm() {
    use conduit_speech::intent_sources::{IntentSourceMaterial, ResolvedIntentSource};
    let source = occurrence::intent([
        segment(
            0,
            SpeechPositionSpecification::known(SpeechWordPosition::Initial).unwrap(),
            SpeechDurationSpecification::known(30, 1).unwrap(),
        ),
        boundary(),
        segment(
            1,
            SpeechPositionSpecification::known(SpeechWordPosition::Medial).unwrap(),
            SpeechDurationSpecification::known(30, 1).unwrap(),
        ),
    ]);
    let material = source_text("source revision");
    let inventory = inventory();
    let voice = voice(&inventory);
    let boundaries = boundaries();
    let context = Context::new();
    let policy = policy(true, true);
    let prepared = prepare_sourced_contextual_intent(
        &source,
        &[IntentSourceMaterial::Text(&material); 3],
        &inventory,
        &voice,
        &boundaries,
        &policy,
        &[context.get(), None, context.get()],
    )
    .unwrap();
    assert!(core::ptr::eq(prepared.source(), &source));
    assert!(core::ptr::eq(
        prepared.sources().intent(),
        prepared.realization().source()
    ));
    for (event, receipt) in prepared.sources().receipts().iter().enumerate() {
        assert_eq!(receipt.location().event, event);
        assert_eq!(receipt.location().source, 0);
        let ResolvedIntentSource::Text(resolved) = receipt.resolved() else {
            panic!()
        };
        assert!(core::ptr::eq(resolved.material(), &material));
        assert_eq!(resolved.text(), "t");
        let reference = match &source.events().as_slice()[event] {
            SpeechUtteranceIntentEvent::Segment(segment) => &segment.sources().as_slice()[0],
            SpeechUtteranceIntentEvent::Boundary(boundary) => &boundary.sources().as_slice()[0],
        };
        assert!(core::ptr::eq(resolved.reference(), reference));
    }
    let expected = pcm(prepared.realization().renderer().unwrap(), 128);
    assert_eq!(pcm(prepared.renderer().unwrap(), 1), expected);
    assert!(expected[266..533].iter().all(|sample| *sample == 0));
}
#[test]
fn stale_late_source_and_missing_coverage_cannot_return_a_playable_result() {
    use conduit_speech::intent_sources::{IntentSourceMaterial, IntentSourcesRefusal};
    let source = occurrence::intent([
        segment(
            0,
            SpeechPositionSpecification::unknown(),
            SpeechDurationSpecification::unknown(),
        ),
        boundary(),
    ]);
    let material = source_text("source revision");
    let stale = source_text("stale revision");
    let inventory = inventory();
    let voice = voice(&inventory);
    let boundaries = boundaries();
    let context = Context::new();
    let policy = policy(true, true);
    assert!(matches!(
        prepare_sourced_contextual_intent(
            &source,
            &[],
            &inventory,
            &voice,
            &boundaries,
            &policy,
            &[context.get(), None]
        ),
        Err(SourcedContextualRefusal::Sources(
            IntentSourcesRefusal::MaterialCount {
                expected: 2,
                supplied: 0
            }
        ))
    ));
    let Err(SourcedContextualRefusal::Sources(IntentSourcesRefusal::Source { location, .. })) =
        prepare_sourced_contextual_intent(
            &source,
            &[
                IntentSourceMaterial::Text(&material),
                IntentSourceMaterial::Text(&stale),
            ],
            &inventory,
            &voice,
            &boundaries,
            &policy,
            &[context.get(), None],
        )
    else {
        panic!()
    };
    assert_eq!(location.event, 1);
    assert_eq!(location.source, 0);
    let Err(SourcedContextualRefusal::Contextual(ContextualIntentRefusal::Timing(_))) =
        prepare_sourced_contextual_intent(
            &source,
            &[IntentSourceMaterial::Text(&material); 2],
            &inventory,
            &voice,
            &boundaries,
            &policy,
            &[context.get(), None],
        )
    else {
        panic!()
    };
}
