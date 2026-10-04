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
    let defaults: Vec<_> = intent
        .events()
        .as_slice()
        .iter()
        .map(|event| match event {
            SpeechUtteranceIntentEvent::Segment(segment) => Some(
                SpeechOccurrenceFeatureObservation::new(
                    SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
                    segment.occurrence().clone(),
                    SpeechEvidenceProvenance::new(
                        "manual empty default realization features".into(),
                        SpeechEvidenceSource::Manual,
                        None,
                    )
                    .unwrap(),
                )
                .unwrap(),
            ),
            SpeechUtteranceIntentEvent::Boundary(_) => None,
        })
        .collect();
    let evidence: Vec<_> = defaults
        .iter()
        .map(|default| {
            default.as_ref().map(|default_features| {
                conduit_speech::global_intent_realization::GlobalSegmentPreparation {
                    context: ExplicitAllophoneContext {
                        careful_style: &style,
                        syllable_position: &syllable,
                        prosodic_context: &prosody,
                    },
                    observed_features: None,
                    default_features,
                }
            })
        })
        .collect();
    let prepared = conduit_speech::global_intent_realization::prepare_global_intent(
        intent,
        inventory,
        voice,
        &boundaries,
        &rules,
        &policy,
        &evidence,
    )
    .map_err(|reason| format!("{reason:?}"))?;
    super::super::write_rendered(
        output,
        "global-intent-initial-aspiration-tata",
        prepared
            .renderer()
            .map_err(|reason| format!("{reason:?}"))?,
    )?;
    let default_rules = SpeechAllophoneRuleProfile::new(
        rules.inventory_id().clone(),
        rules.language().clone(),
        BoundedSequence::new(),
    )
    .unwrap();
    let default_policy =
        SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false).unwrap();
    let mut default_events = Vec::with_capacity(evidence.len());
    for (event, supplied) in evidence.iter().enumerate() {
        let supplied = supplied
            .as_ref()
            .ok_or("default fixture requires segment evidence")?;
        let choice = select_global_allophone_rule(
            intent,
            event,
            &default_rules,
            &default_policy,
            supplied.observed_features,
            supplied.context,
        )
        .map_err(|reason| format!("{reason:?}"))?;
        let selected =
            conduit_speech::global_default_choice::finish_global_default_choice(&choice, inventory)
                .map_err(|reason| format!("{reason:?}"))?;
        let admitted = conduit_speech::global_default_profile::prepare_global_default_profile(
            &selected,
            voice,
            supplied.default_features,
        )
        .map_err(|reason| format!("{reason:?}"))?;
        default_events.push(admitted.event());
    }
    super::super::write_rendered(
        output,
        "global-declared-default-tata",
        timing
            .renderer(&default_events)
            .map_err(|reason| format!("{reason:?}"))?,
    )?;
    let default_intent = conduit_speech::global_intent_realization::prepare_global_intent(
        intent,
        inventory,
        voice,
        &boundaries,
        &default_rules,
        &default_policy,
        &evidence,
    )
    .map_err(|reason| format!("{reason:?}"))?;
    super::super::write_rendered(
        output,
        "global-default-intent-tata",
        default_intent
            .renderer()
            .map_err(|reason| format!("{reason:?}"))?,
    )?;
    Ok(())
}
