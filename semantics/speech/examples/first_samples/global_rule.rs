//! Explicit manual global-rule fixture; quantitative timing remains native.
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    chosen_global_rule_profile::prepare_chosen_global_rule_profile,
    declared_context::ExplicitAllophoneContext,
    global_rule_selection::select_global_allophone_rule, semantic::*,
    utterance_timing::prepare_utterance_timing,
};
fn rule(
    id: &str,
    phoneme: &str,
    phone: &str,
    position: SpeechPositionSpecification,
) -> SpeechAllophoneRule {
    SpeechAllophoneRule::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::unspecified(),
            StressSpecification::unspecified(),
            SpeechSyllablePositionSpecification::unspecified(),
            position,
        )
        .unwrap(),
        id.into(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneSpecification::known(PhoneId::new(phone.into()).unwrap()).unwrap(),
        PhonemeSpecification::known(PhonemeId::new(phoneme.into()).unwrap()).unwrap(),
        SpeechRuleStatus::Productive,
    )
    .unwrap()
}
pub fn write(
    output: &str,
    intent: &SpeechUtteranceIntent,
    inventory: &SpeechInventory,
    voice: &SpeechFormantVoiceProfile,
) -> Result<(), Box<dyn std::error::Error>> {
    let rules = SpeechAllophoneRuleProfile::new(
        inventory.identity().clone(),
        inventory.language().clone(),
        BoundedSequence::try_from_iter([
            rule(
                "word-initial aspiration",
                "choice/phoneme/t",
                "choice/aspirated-t",
                SpeechPositionSpecification::known(SpeechWordPosition::Initial).unwrap(),
            ),
            rule(
                "plain t",
                "choice/phoneme/t",
                "choice/t",
                SpeechPositionSpecification::unspecified(),
            ),
            rule(
                "plain aa",
                "choice/phoneme/aa",
                "choice/aa",
                SpeechPositionSpecification::unspecified(),
            ),
        ])
        .unwrap(),
    )
    .unwrap();
    let policy = SpeechAllophoneChoicePolicy::new(false, false, false, false, true, false).unwrap();
    let style = SpeechCarefulStyleSpecification::known(false).unwrap();
    let syllable = SpeechSyllablePositionSpecification::unknown();
    let prosody = SpeechProsodicContextSpecification::unknown();
    let mut events = Vec::with_capacity(intent.events().as_slice().len());
    for event in 0..intent.events().as_slice().len() {
        let choice = select_global_allophone_rule(
            intent,
            event,
            &rules,
            &policy,
            None,
            ExplicitAllophoneContext {
                careful_style: &style,
                syllable_position: &syllable,
                prosodic_context: &prosody,
            },
        )
        .map_err(|reason| format!("{reason:?}"))?;
        let default = SpeechOccurrenceFeatureObservation::new(
            SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
            choice.occurrence().segment().occurrence().clone(),
            SpeechEvidenceProvenance::new(
                "manual empty default realization features".into(),
                SpeechEvidenceSource::Manual,
                None,
            )
            .unwrap(),
        )
        .unwrap();
        let prepared = prepare_chosen_global_rule_profile(&choice, inventory, voice, &default)
            .map_err(|reason| format!("{reason:?}"))?;
        println!(
            "global-rule-tata event {event}: {:?}, {:?}",
            choice.state().outcome(),
            prepared.definition().identity()
        );
        events.push(prepared.event());
    }
    let boundaries = SpeechFormantBoundaryProfile::new(BoundedSequence::new()).unwrap();
    let timing =
        prepare_utterance_timing(intent, &boundaries).map_err(|reason| format!("{reason:?}"))?;
    super::super::write_rendered(
        output,
        "global-rule-initial-aspiration-tata",
        timing
            .renderer(&events)
            .map_err(|reason| format!("{reason:?}"))?,
    )?;
    Ok(())
}
