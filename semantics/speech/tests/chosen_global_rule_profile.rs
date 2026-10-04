#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    chosen_global_rule_profile::*, declared_context::ExplicitAllophoneContext,
    global_rule_selection::select_global_allophone_rule, profile_admission::ProfileRefusal,
    semantic::*, Renderer, VoiceEvent,
};
#[allow(dead_code)]
#[path = "common/native_phone.rs"]
mod native;
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod occurrence;
fn empty() -> SpeechFeatureBundle {
    SpeechFeatureBundle::new(BoundedSequence::new()).unwrap()
}
fn features() -> SpeechFeatureBundle {
    SpeechFeatureBundle::new(
        BoundedSequence::try_from_iter([SpeechFeature::new(
            SpeechFeatureId::new("voice".into()).unwrap(),
            FeatureSpecification::unknown(),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
fn rules(phone: PhoneSpecification, features: SpeechFeatureBundle) -> SpeechAllophoneRuleProfile {
    let rule = SpeechAllophoneRule::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unspecified(),
            StressSpecification::unspecified(),
            SpeechSyllablePositionSpecification::unspecified(),
            SpeechPositionSpecification::unspecified(),
        )
        .unwrap(),
        "original rule".into(),
        empty(),
        features,
        phone,
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
fn requested_phone() -> PhoneSpecification {
    PhoneSpecification::known(native::id("phone/t")).unwrap()
}
fn voice(definition: SpeechPhone) -> SpeechFormantVoiceProfile {
    SpeechFormantVoiceProfile::new(
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
    .unwrap()
}
fn default(ordinal: u32, features: SpeechFeatureBundle) -> SpeechOccurrenceFeatureObservation {
    SpeechOccurrenceFeatureObservation::new(
        features,
        occurrence::token(ordinal, occurrence::BASIS),
        SpeechEvidenceProvenance::new(
            "explicit default feature fixture".into(),
            SpeechEvidenceSource::Manual,
            None,
        )
        .unwrap(),
    )
    .unwrap()
}
struct Explicit {
    style: SpeechCarefulStyleSpecification,
    syllable: SpeechSyllablePositionSpecification,
    prosody: SpeechProsodicContextSpecification,
    policy: SpeechAllophoneChoicePolicy,
}
impl Explicit {
    fn new() -> Self {
        Self {
            style: SpeechCarefulStyleSpecification::known(false).unwrap(),
            syllable: SpeechSyllablePositionSpecification::unknown(),
            prosody: SpeechProsodicContextSpecification::unknown(),
            policy: SpeechAllophoneChoicePolicy::new(false, false, false, false, true, false)
                .unwrap(),
        }
    }
    fn get(&self) -> ExplicitAllophoneContext<'_> {
        ExplicitAllophoneContext {
            careful_style: &self.style,
            syllable_position: &self.syllable,
            prosodic_context: &self.prosody,
        }
    }
}
fn pcm(event: VoiceEvent, block: usize) -> Vec<i16> {
    let events = [event];
    let mut renderer = Renderer::prepare(&events).unwrap();
    let mut pcm = Vec::new();
    let mut buffer = [0; 128];
    while !renderer.is_complete() {
        let count = renderer.render(&mut buffer[..block]).unwrap();
        pcm.extend_from_slice(&buffer[..count]);
    }
    pcm
}
#[test]
fn selected_rule_binds_exact_definition_and_profile_with_original_default_evidence() {
    let intent = occurrence::intent([occurrence::segment(10)]);
    let rules = rules(requested_phone(), empty());
    let explicit = Explicit::new();
    let inventory = native::inventory("inventory", "en", vec![native::definition("phone/t")]);
    let voice = voice(native::definition("phone/t"));
    let default = default(10, empty());
    let before = intent.clone();
    let choice =
        select_global_allophone_rule(&intent, 0, &rules, &explicit.policy, None, explicit.get())
            .unwrap();
    let prepared =
        prepare_chosen_global_rule_profile(&choice, &inventory, &voice, &default).unwrap();
    assert!(core::ptr::eq(prepared.choice(), &choice));
    assert!(core::ptr::eq(prepared.inventory(), &inventory));
    assert!(core::ptr::eq(prepared.default_features(), &default));
    assert!(core::ptr::eq(
        prepared.definition(),
        &inventory.phones().as_slice()[0]
    ));
    assert!(core::ptr::eq(
        prepared.binding(),
        &voice.phones().as_slice()[0]
    ));
    assert!(core::ptr::eq(prepared.profile(), &voice));
    assert!(core::ptr::eq(
        prepared.features().rule(),
        choice.selected_rule().unwrap()
    ));
    assert_eq!(prepared.features().features().count(), 0);
    assert_eq!(
        prepared.checked_default_occurrence().expected(),
        default.occurrence()
    );
    assert_eq!(
        prepared.checked_default_occurrence().observed(),
        default.occurrence()
    );
    let expected = VoiceEvent::phone(conduit_speech::SpeechPhoneInput {
        phone: conduit_speech::EnglishPhone::t,
        stress: conduit_speech::EnglishStress::primary,
    });
    assert_eq!(pcm(prepared.event(), 1), pcm(expected, 128));
    assert_eq!(intent, before);
}
#[test]
fn foreign_basis_occurrence_missing_duplicate_and_stale_definitions_refuse() {
    let intent = occurrence::intent([occurrence::segment(10)]);
    let rules = rules(requested_phone(), empty());
    let explicit = Explicit::new();
    let voice = voice(native::definition("phone/t"));
    let default = default(10, empty());
    let choice =
        select_global_allophone_rule(&intent, 0, &rules, &explicit.policy, None, explicit.get())
            .unwrap();
    for (id, language) in [("foreign", "en"), ("inventory", "foreign")] {
        let inventory = native::inventory(id, language, vec![native::definition("phone/t")]);
        assert!(matches!(
            prepare_chosen_global_rule_profile(&choice, &inventory, &voice, &default),
            Err(ChosenGlobalProfileRefusal::InventoryBasis(_))
        ));
    }
    let inventory = native::inventory("inventory", "en", vec![native::definition("phone/t")]);
    let foreign = self::default(11, empty());
    assert!(matches!(
        prepare_chosen_global_rule_profile(&choice, &inventory, &voice, &foreign),
        Err(ChosenGlobalProfileRefusal::DefaultOccurrence(_))
    ));
    let missing = native::inventory("inventory", "en", vec![]);
    assert!(matches!(
        prepare_chosen_global_rule_profile(&choice, &missing, &voice, &default),
        Err(ChosenGlobalProfileRefusal::MissingDefinition)
    ));
    let duplicate = native::inventory(
        "inventory",
        "en",
        vec![native::definition("phone/t"), native::definition("phone/t")],
    );
    assert!(matches!(
        prepare_chosen_global_rule_profile(&choice, &duplicate, &voice, &default),
        Err(ChosenGlobalProfileRefusal::AmbiguousDefinition)
    ));
    let stale = SpeechPhone::new(
        BoundedSequence::new(),
        empty(),
        native::id("phone/t"),
        "t".into(),
        SpeechSegmentStatus::Core,
    )
    .unwrap();
    let stale_voice = self::voice(stale);
    assert!(matches!(
        prepare_chosen_global_rule_profile(&choice, &inventory, &stale_voice, &default),
        Err(ChosenGlobalProfileRefusal::Profile(
            ProfileRefusal::DefinitionSnapshot
        ))
    ));
}
#[test]
fn inherited_features_are_refused_instead_of_discarded_by_compact_voice() {
    let intent = occurrence::intent([occurrence::segment(10)]);
    let explicit = Explicit::new();
    let inventory = native::inventory("inventory", "en", vec![native::definition("phone/t")]);
    let voice = voice(native::definition("phone/t"));
    for (output, defaults) in [(features(), empty()), (empty(), features())] {
        let rules = rules(requested_phone(), output);
        let default = default(10, defaults);
        let choice = select_global_allophone_rule(
            &intent,
            0,
            &rules,
            &explicit.policy,
            None,
            explicit.get(),
        )
        .unwrap();
        assert!(matches!(
            prepare_chosen_global_rule_profile(&choice, &inventory, &voice, &default),
            Err(ChosenGlobalProfileRefusal::Profile(
                ProfileRefusal::UnsupportedTokenFeatures
            ))
        ));
    }
    let rich = SpeechPhone::new(
        BoundedSequence::new(),
        features(),
        native::id("phone/t"),
        "t".into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap();
    let inventory = native::inventory("inventory", "en", vec![rich.clone()]);
    let voice = self::voice(rich);
    let default = default(10, empty());
    let rules = rules(requested_phone(), empty());
    let choice =
        select_global_allophone_rule(&intent, 0, &rules, &explicit.policy, None, explicit.get())
            .unwrap();
    assert!(matches!(
        prepare_chosen_global_rule_profile(&choice, &inventory, &voice, &default),
        Err(ChosenGlobalProfileRefusal::Profile(
            ProfileRefusal::UnsupportedDefinitionFeatures
        ))
    ));
}
#[test]
fn deferral_and_selected_unknown_output_remain_distinct_typed_refusals() {
    let known_intent = occurrence::intent([occurrence::segment(10)]);
    let rules = rules(PhoneSpecification::unknown(), empty());
    let explicit = Explicit::new();
    let inventory = native::inventory("inventory", "en", vec![native::definition("phone/t")]);
    let voice = voice(native::definition("phone/t"));
    let default = default(10, empty());
    let deferred = select_global_allophone_rule(
        &known_intent,
        0,
        &rules,
        &explicit.policy,
        None,
        explicit.get(),
    )
    .unwrap();
    assert!(
        matches!(prepare_chosen_global_rule_profile(&deferred, &inventory, &voice, &default), Err(ChosenGlobalProfileRefusal::Unchosen(state)) if state == *deferred.state())
    );
    let SpeechUtteranceIntentEvent::Segment(segment) = occurrence::segment(10) else {
        unreachable!()
    };
    let intent = occurrence::intent([SpeechUtteranceIntentEvent::segment(
        segment.occurrence().clone(),
        PhoneSpecification::unspecified(),
        segment.phoneme().clone(),
        segment.prosody().clone(),
        segment.provenance().clone(),
        segment.sources().clone(),
        segment.stress().clone(),
        segment.word_position().clone(),
    )
    .unwrap()]);
    let selected =
        select_global_allophone_rule(&intent, 0, &rules, &explicit.policy, None, explicit.get())
            .unwrap();
    assert!(
        matches!(prepare_chosen_global_rule_profile(&selected, &inventory, &voice, &default), Err(ChosenGlobalProfileRefusal::UnresolvedOutput(value)) if core::ptr::eq(value, rules.rules().as_slice()[0].phone()))
    );
}
