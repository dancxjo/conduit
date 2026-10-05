#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    chosen_global_rule_profile::ChosenGlobalProfileRefusal,
    declared_context::ExplicitAllophoneContext, global_intent_realization::*, semantic::*,
    EnglishStress, Renderer, SpeechPhoneInput, VoiceEvent,
};
#[allow(dead_code)]
#[path = "common/native_phone.rs"]
mod native;
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod occurrence;
fn segment(ordinal: u32) -> SpeechUtteranceIntentEvent {
    let SpeechUtteranceIntentEvent::Segment(original) = occurrence::segment(ordinal) else {
        unreachable!()
    };
    SpeechUtteranceIntentEvent::segment(
        original.occurrence().clone(),
        original.phone().clone(),
        original.phoneme().clone(),
        SpeechSegmentProsodyIntent::new(
            SpeechDurationSpecification::known(100, 1).unwrap(),
            SpeechCycleSpecification::known(120, 1).unwrap(),
            SpeechIntensitySpecification::known(1, 1).unwrap(),
        )
        .unwrap(),
        original.provenance().clone(),
        original.sources().clone(),
        original.stress().clone(),
        original.word_position().clone(),
    )
    .unwrap()
}
fn boundary() -> SpeechUtteranceIntentEvent {
    let SpeechUtteranceIntentEvent::Boundary(original) =
        occurrence::boundary(SpeechBoundarySpecification::known(SpeechBoundaryKind::Word).unwrap())
    else {
        unreachable!()
    };
    SpeechUtteranceIntentEvent::boundary(
        SpeechDurationSpecification::known(100, 1).unwrap(),
        original.kind().clone(),
        original.provenance().clone(),
        original.sources().clone(),
    )
    .unwrap()
}
fn rules(stress: StressSpecification) -> SpeechAllophoneRuleProfile {
    let rule = SpeechAllophoneRule::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unspecified(),
            stress,
            SpeechSyllablePositionSpecification::unspecified(),
            SpeechPositionSpecification::unspecified(),
        )
        .unwrap(),
        "original rule".into(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneSpecification::known(native::id("phone/t")).unwrap(),
        PhonemeSpecification::unspecified(),
        SpeechRuleStatus::Productive,
    )
    .unwrap();
    SpeechAllophoneRuleProfile::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        BoundedSequence::try_from_iter([rule]).unwrap(),
    )
    .unwrap()
}
fn default(ordinal: u32) -> SpeechOccurrenceFeatureObservation {
    SpeechOccurrenceFeatureObservation::new(
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        occurrence::token(ordinal, occurrence::BASIS),
        SpeechEvidenceProvenance::new(
            "explicit empty defaults".into(),
            SpeechEvidenceSource::Manual,
            None,
        )
        .unwrap(),
    )
    .unwrap()
}
fn inventory() -> SpeechInventory {
    let phoneme = SpeechPhoneme::new(
        BoundedSequence::new(),
        BoundedSequence::new(),
        Some(native::id("phone/t")),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeId::new("phoneme/t".into()).unwrap(),
        "t".into(),
        BoundedSequence::try_from_iter([native::id("phone/t")]).unwrap(),
        SpeechSegmentStatus::Core,
    )
    .unwrap();
    SpeechInventory::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        BoundedSequence::try_from_iter([phoneme]).unwrap(),
        BoundedSequence::try_from_iter([native::definition("phone/t")]).unwrap(),
    )
    .unwrap()
}
struct Setup {
    inventory: SpeechInventory,
    voice: SpeechFormantVoiceProfile,
    boundaries: SpeechFormantBoundaryProfile,
    rules: SpeechAllophoneRuleProfile,
    policy: SpeechAllophoneChoicePolicy,
    style: SpeechCarefulStyleSpecification,
    syllable: SpeechSyllablePositionSpecification,
    prosody: SpeechProsodicContextSpecification,
}
impl Setup {
    fn new() -> Self {
        let definition = native::definition("phone/t");
        Self {
            inventory: inventory(),
            voice: SpeechFormantVoiceProfile::new(
                "voice".into(),
                SpeechInventoryId::new("inventory".into()).unwrap(),
                SpeechLanguageId::new("en".into()).unwrap(),
                BoundedSequence::try_from_iter([SpeechFormantPhoneBinding::new(
                    definition,
                    EnglishPhone::T,
                )
                .unwrap()])
                .unwrap(),
            )
            .unwrap(),
            boundaries: SpeechFormantBoundaryProfile::new(
                BoundedSequence::try_from_iter([SpeechFormantBoundaryBinding::new(
                    SpeechBoundaryKind::Word,
                    SpeechFormantBoundary::Word,
                )
                .unwrap()])
                .unwrap(),
            )
            .unwrap(),
            rules: rules(StressSpecification::unspecified()),
            policy: SpeechAllophoneChoicePolicy::new(false, false, false, false, true, false)
                .unwrap(),
            style: SpeechCarefulStyleSpecification::known(false).unwrap(),
            syllable: SpeechSyllablePositionSpecification::unknown(),
            prosody: SpeechProsodicContextSpecification::unknown(),
        }
    }
    fn evidence<'a>(
        &'a self,
        default_features: &'a SpeechOccurrenceFeatureObservation,
    ) -> Option<GlobalSegmentPreparation<'a>> {
        Some(GlobalSegmentPreparation {
            context: ExplicitAllophoneContext {
                careful_style: &self.style,
                syllable_position: &self.syllable,
                prosodic_context: &self.prosody,
            },
            observed_features: None,
            default_features,
        })
    }
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
fn frozen_global_choices_timing_and_boundary_preserve_original_receipts_and_pcm() {
    let source = occurrence::intent([segment(10), boundary(), segment(11)]);
    let before = source.clone();
    let setup = Setup::new();
    let defaults = [default(10), default(11)];
    let prepared = prepare_global_intent(
        &source,
        &setup.inventory,
        &setup.voice,
        &setup.boundaries,
        &setup.rules,
        &setup.policy,
        &[
            setup.evidence(&defaults[0]),
            None,
            setup.evidence(&defaults[1]),
        ],
    )
    .unwrap();
    assert!(core::ptr::eq(prepared.source(), &source));
    assert!(core::ptr::eq(prepared.inventory(), &setup.inventory));
    assert!(core::ptr::eq(prepared.profile(), &setup.voice));
    assert!(core::ptr::eq(prepared.rules(), &setup.rules));
    assert_eq!(prepared.phones().len(), 2);
    assert_eq!(prepared.events().len(), 3);
    for (receipt, (event, default)) in prepared
        .phones()
        .iter()
        .zip([(0, &defaults[0]), (2, &defaults[1])])
    {
        assert_eq!(receipt.event_index(), event);
        assert!(core::ptr::eq(
            receipt.phoneme().definition(),
            &setup.inventory.phonemes().as_slice()[0]
        ));
        assert!(core::ptr::eq(
            receipt.phoneme().occurrence().segment(),
            receipt.choice().occurrence().segment()
        ));
        assert!(core::ptr::eq(receipt.default_features(), default));
        let SpeechUtteranceIntentEvent::Segment(original) = &source.events().as_slice()[event]
        else {
            unreachable!()
        };
        assert!(core::ptr::eq(
            receipt.choice().occurrence().segment(),
            original
        ));
        assert!(core::ptr::eq(
            receipt.features().unwrap().rule(),
            &setup.rules.rules().as_slice()[0]
        ));
        assert_eq!(
            receipt.checked_default_occurrence().expected(),
            original.occurrence()
        );
        assert_eq!(
            receipt.checked_default_occurrence().observed(),
            default.occurrence()
        );
        assert!(core::ptr::eq(
            receipt.definition(),
            &setup.inventory.phones().as_slice()[0]
        ));
        assert!(core::ptr::eq(
            receipt.binding(),
            &setup.voice.phones().as_slice()[0]
        ));
        assert_eq!(receipt.features().unwrap().features().count(), 0);
    }
    let phone = VoiceEvent::phone(SpeechPhoneInput {
        phone: conduit_speech::EnglishPhone::t,
        stress: EnglishStress::primary,
    });
    let expected = [
        phone,
        VoiceEvent::boundary(conduit_speech::VoiceBoundary::word),
        phone,
    ];
    assert_eq!(prepared.events(), expected);
    let reference = pcm(prepared.timing().renderer(&expected).unwrap(), 128);
    assert_eq!(pcm(prepared.renderer().unwrap(), 1), reference);
    assert_eq!(pcm(prepared.renderer().unwrap(), 63), reference);
    let frames = conduit_speech::SAMPLE_RATE_HZ as usize / 100;
    assert_eq!(reference.len(), frames * 3);
    assert!(reference[frames..frames * 2]
        .iter()
        .all(|sample| *sample == 0));
    assert_eq!(source, before);
}
#[test]
fn evidence_alignment_refuses_before_timing_or_choice() {
    let source = occurrence::intent([occurrence::segment(10), boundary(), occurrence::segment(11)]);
    let setup = Setup::new();
    let defaults = [default(10), default(11)];
    assert!(matches!(
        prepare_global_intent(
            &source,
            &setup.inventory,
            &setup.voice,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &[]
        ),
        Err(GlobalIntentRefusal::EvidenceCount)
    ));
    assert!(matches!(
        prepare_global_intent(
            &source,
            &setup.inventory,
            &setup.voice,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &[setup.evidence(&defaults[0]), None, None]
        ),
        Err(GlobalIntentRefusal::MissingSegmentEvidence { event: 2 })
    ));
    assert!(matches!(
        prepare_global_intent(
            &source,
            &setup.inventory,
            &setup.voice,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &[
                setup.evidence(&defaults[0]),
                setup.evidence(&defaults[0]),
                setup.evidence(&defaults[1])
            ]
        ),
        Err(GlobalIntentRefusal::BoundaryEvidence { event: 1 })
    ));
}
#[test]
fn late_foreign_default_or_unselected_rule_cannot_return_a_renderable_result() {
    let source = occurrence::intent([segment(10), segment(11)]);
    let before = source.clone();
    let setup = Setup::new();
    let defaults = [default(10), default(999)];
    assert!(matches!(
        prepare_global_intent(
            &source,
            &setup.inventory,
            &setup.voice,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &[setup.evidence(&defaults[0]), setup.evidence(&defaults[1])]
        ),
        Err(GlobalIntentRefusal::Profile {
            event: 1,
            reason: ChosenGlobalProfileRefusal::DefaultOccurrence(_)
        })
    ));
    let exact_default = default(11);
    let mut foreign_observation = setup.evidence(&exact_default).unwrap();
    foreign_observation.observed_features = Some(&defaults[1]);
    assert!(
        matches!(prepare_global_intent(&source, &setup.inventory, &setup.voice, &setup.boundaries, &setup.rules, &setup.policy,
        &[setup.evidence(&defaults[0]), Some(foreign_observation)]),
        Err(GlobalIntentRefusal::Choice { event: 1, reason })
        if matches!(reason.as_ref(), conduit_speech::global_rule_selection::GlobalRuleSelectionRefusal::FeatureOccurrence(_)))
    );
    let excluded =
        SpeechAllophoneChoicePolicy::new(false, false, false, false, false, false).unwrap();
    assert!(
        matches!(prepare_global_intent(&source, &setup.inventory, &setup.voice, &setup.boundaries, &setup.rules, &excluded, &[setup.evidence(&defaults[0]), setup.evidence(&defaults[0])]),
        Err(GlobalIntentRefusal::Profile { event: 0, reason: ChosenGlobalProfileRefusal::Unchosen(state) }) if state.outcome() == &SpeechAllophoneChoiceOutcome::None)
    );
    assert_eq!(source, before);
}

#[test]
fn deferred_choice_preserves_its_exact_reason_in_the_utterance_refusal() {
    let source = occurrence::intent([segment(10)]);
    let setup = Setup::new();
    let defaults = default(10);
    let unresolved = rules(StressSpecification::unknown());
    assert!(
        matches!(prepare_global_intent(&source, &setup.inventory, &setup.voice, &setup.boundaries, &unresolved, &setup.policy, &[setup.evidence(&defaults)]),
        Err(GlobalIntentRefusal::Profile { event: 0, reason: ChosenGlobalProfileRefusal::Unchosen(state) })
        if state.outcome() == &SpeechAllophoneChoiceOutcome::Deferred && state.reason() == &SpeechContextDecision::RequirementUnresolved)
    );
}

#[test]
fn late_missing_phoneme_refuses_even_when_a_wildcard_rule_and_phone_would_match() {
    let first = segment(10);
    let SpeechUtteranceIntentEvent::Segment(original) = segment(11) else {
        unreachable!()
    };
    let second = SpeechUtteranceIntentEvent::segment(
        original.occurrence().clone(),
        original.phone().clone(),
        PhonemeSpecification::known(PhonemeId::new("phoneme/foreign".into()).unwrap()).unwrap(),
        original.prosody().clone(),
        original.provenance().clone(),
        original.sources().clone(),
        original.stress().clone(),
        original.word_position().clone(),
    )
    .unwrap();
    let source = occurrence::intent([first, second]);
    let setup = Setup::new();
    let defaults = [default(10), default(11)];
    assert!(
        matches!(prepare_global_intent(&source, &setup.inventory, &setup.voice, &setup.boundaries, &setup.rules, &setup.policy,
        &[setup.evidence(&defaults[0]), setup.evidence(&defaults[1])]),
        Err(GlobalIntentRefusal::Phoneme { event: 1, reason })
        if matches!(reason.as_ref(), conduit_speech::intent_phoneme_inventory::IntentPhonemeRefusal::MissingDefinition))
    );
}

#[test]
fn declared_global_default_preserves_policy_constraints_winners_and_deferral() {
    use conduit_speech::{
        global_default_choice::*, global_rule_selection::select_global_allophone_rule,
    };
    let source = occurrence::intent([segment(10)]);
    let setup = Setup::new();
    let default = default(10);
    let context = setup.evidence(&default).unwrap().context;
    let empty = SpeechAllophoneRuleProfile::new(
        setup.rules.inventory_id().clone(),
        setup.rules.language().clone(),
        BoundedSequence::new(),
    )
    .unwrap();
    let allowed = SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false).unwrap();
    let choice = select_global_allophone_rule(&source, 0, &empty, &allowed, None, context).unwrap();
    let finished = finish_global_default_choice(&choice, &setup.inventory).unwrap();
    assert_eq!(
        finished.state().outcome(),
        &SpeechAllophoneChoiceOutcome::SelectedDefault
    );
    assert!(core::ptr::eq(finished.choice(), &choice));
    assert!(core::ptr::eq(
        finished.phoneme().occurrence().intent(),
        &source
    ));
    assert!(core::ptr::eq(
        finished.selected_default().unwrap(),
        setup.inventory.phonemes().as_slice()[0]
            .default_phone()
            .as_ref()
            .unwrap()
    ));
    let forbidden =
        select_global_allophone_rule(&source, 0, &empty, &setup.policy, None, context).unwrap();
    assert_eq!(
        finish_global_default_choice(&forbidden, &setup.inventory)
            .unwrap()
            .state()
            .outcome(),
        &SpeechAllophoneChoiceOutcome::None
    );
    for profile in [
        rules(StressSpecification::unspecified()),
        rules(StressSpecification::unknown()),
    ] {
        let choice =
            select_global_allophone_rule(&source, 0, &profile, &allowed, None, context).unwrap();
        let finished = finish_global_default_choice(&choice, &setup.inventory).unwrap();
        assert_eq!(finished.state(), choice.state());
        assert!(finished.selected_default().is_none());
    }
    let SpeechUtteranceIntentEvent::Segment(original) = segment(10) else {
        unreachable!()
    };
    for requested in ["phone/t", "phone/foreign"] {
        let constrained = occurrence::intent([SpeechUtteranceIntentEvent::segment(
            original.occurrence().clone(),
            PhoneSpecification::known(native::id(requested)).unwrap(),
            original.phoneme().clone(),
            original.prosody().clone(),
            original.provenance().clone(),
            original.sources().clone(),
            original.stress().clone(),
            original.word_position().clone(),
        )
        .unwrap()]);
        let choice =
            select_global_allophone_rule(&constrained, 0, &empty, &allowed, None, context).unwrap();
        let finished = finish_global_default_choice(&choice, &setup.inventory).unwrap();
        assert_eq!(
            finished.selected_default().is_some(),
            requested == "phone/t"
        );
        assert_eq!(
            finished.checked_default_identity().is_some(),
            requested == "phone/t"
        );
    }
}

#[test]
fn selected_default_voice_retains_exact_inputs_and_refuses_foreign_defaults() {
    use conduit_speech::{
        global_default_choice::*, global_default_profile::*,
        global_rule_selection::select_global_allophone_rule,
    };
    let source = occurrence::intent([segment(10)]);
    let setup = Setup::new();
    let defaults = default(10);
    let context = setup.evidence(&defaults).unwrap().context;
    let empty = SpeechAllophoneRuleProfile::new(
        setup.rules.inventory_id().clone(),
        setup.rules.language().clone(),
        BoundedSequence::new(),
    )
    .unwrap();
    let allowed = SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false).unwrap();
    let choice = select_global_allophone_rule(&source, 0, &empty, &allowed, None, context).unwrap();
    let selected = finish_global_default_choice(&choice, &setup.inventory).unwrap();
    let prepared = prepare_global_default_profile(&selected, &setup.voice, &defaults).unwrap();
    assert!(core::ptr::eq(prepared.choice(), &selected));
    assert!(core::ptr::eq(
        prepared.definition(),
        &setup.inventory.phones().as_slice()[0]
    ));
    assert!(core::ptr::eq(
        prepared.binding(),
        &setup.voice.phones().as_slice()[0]
    ));
    assert!(core::ptr::eq(prepared.default_features(), &defaults));
    let events = [prepared.event()];
    let timing =
        conduit_speech::utterance_timing::prepare_utterance_timing(&source, &setup.boundaries)
            .unwrap();
    assert_eq!(
        pcm(timing.renderer(&events).unwrap(), 1),
        pcm(timing.renderer(&events).unwrap(), 128)
    );
    assert!(matches!(
        prepare_global_default_profile(&selected, &setup.voice, &default(11)),
        Err(GlobalDefaultProfileRefusal::DefaultOccurrence(_))
    ));
    let forbidden =
        select_global_allophone_rule(&source, 0, &empty, &setup.policy, None, context).unwrap();
    let none = finish_global_default_choice(&forbidden, &setup.inventory).unwrap();
    assert!(
        matches!(prepare_global_default_profile(&none, &setup.voice, &defaults), Err(GlobalDefaultProfileRefusal::Unchosen(state)) if state == *none.state())
    );
    let missing = SpeechInventory::new(
        setup.inventory.identity().clone(),
        setup.inventory.language().clone(),
        setup.inventory.phonemes().clone(),
        BoundedSequence::new(),
    )
    .unwrap();
    let selected = finish_global_default_choice(&choice, &missing).unwrap();
    assert!(matches!(
        prepare_global_default_profile(&selected, &setup.voice, &defaults),
        Err(GlobalDefaultProfileRefusal::MissingDefinition)
    ));
    let duplicate = SpeechInventory::new(
        setup.inventory.identity().clone(),
        setup.inventory.language().clone(),
        setup.inventory.phonemes().clone(),
        BoundedSequence::try_from_iter([
            native::definition("phone/t"),
            native::definition("phone/t"),
        ])
        .unwrap(),
    )
    .unwrap();
    let selected = finish_global_default_choice(&choice, &duplicate).unwrap();
    assert!(matches!(
        prepare_global_default_profile(&selected, &setup.voice, &defaults),
        Err(GlobalDefaultProfileRefusal::AmbiguousDefinition)
    ));
}

#[path = "common/global_default_intent_cases.rs"]
mod default_intent_cases;

#[path = "common/sourced_global_intent_cases.rs"]
mod sourced_intent_cases;
