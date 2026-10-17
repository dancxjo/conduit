//! Identical semantic phone selected with two explicitly mapped feature values.
use super::*;
#[allow(clippy::too_many_arguments)]
pub fn write(
    output: &str,
    intent: &SpeechUtteranceIntent,
    inventory: &SpeechInventory,
    voice: &SpeechFormantVoiceProfile,
    boundaries: &SpeechFormantBoundaryProfile,
    policy: &SpeechAllophoneChoicePolicy,
    evidence: &[Option<conduit_speech::global_intent_realization::GlobalSegmentPreparation<'_>>],
) -> Result<(), Box<dyn std::error::Error>> {
    let profile = SpeechFormantAspirationProfile::new(
        SpeechFeatureId::new("explicit aspiration cue".into()).unwrap(),
        voice.clone(),
    )
    .unwrap();
    for enabled in [false, true] {
        let original = rule(
            "feature-controlled initial t",
            "choice/phoneme/t",
            "choice/t",
            SpeechPositionSpecification::known(SpeechWordPosition::Initial).unwrap(),
        );
        let output_features = SpeechFeatureBundle::new(
            BoundedSequence::try_from_iter([SpeechFeature::new(
                profile.feature_id().clone(),
                FeatureSpecification::known(SpeechFeatureValue::boolean(enabled).unwrap()).unwrap(),
            )
            .unwrap()])
            .unwrap(),
        )
        .unwrap();
        let first = SpeechAllophoneRule::new(
            original.conditions().clone(),
            original.confidence().clone(),
            original.environment().clone(),
            original.identity().clone(),
            original.input_features().clone(),
            output_features,
            original.phone().clone(),
            original.phoneme().clone(),
            *original.status(),
        )
        .unwrap();
        let rules = SpeechAllophoneRuleProfile::new(
            inventory.identity().clone(),
            inventory.language().clone(),
            BoundedSequence::try_from_iter([
                first,
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
        let prepared = conduit_speech::global_intent_realization::prepare_aspirated_global_intent(
            intent, inventory, &profile, boundaries, &rules, policy, evidence,
        )
        .map_err(|reason| format!("{reason:?}"))?;
        super::super::super::write_rendered(
            output,
            if enabled {
                "feature-aspiration-on-tata"
            } else {
                "feature-aspiration-off-tata"
            },
            prepared
                .renderer()
                .map_err(|reason| format!("{reason:?}"))?,
        )?;
        let default_rules = SpeechAllophoneRuleProfile::new(
            inventory.identity().clone(),
            inventory.language().clone(),
            BoundedSequence::new(),
        )
        .unwrap();
        let default_policy =
            SpeechAllophoneChoicePolicy::new(true, false, false, false, true, false).unwrap();
        let defaults: Vec<_> = evidence
            .iter()
            .enumerate()
            .map(|(event, supplied)| {
                supplied.map(|supplied| {
                    let original = supplied.default_features;
                    let features = if event == 0 {
                        SpeechFeatureBundle::new(
                            BoundedSequence::try_from_iter([SpeechFeature::new(
                                profile.feature_id().clone(),
                                FeatureSpecification::known(
                                    SpeechFeatureValue::boolean(enabled).unwrap(),
                                )
                                .unwrap(),
                            )
                            .unwrap()])
                            .unwrap(),
                        )
                        .unwrap()
                    } else {
                        original.features().clone()
                    };
                    SpeechOccurrenceFeatureObservation::new(
                        features,
                        original.occurrence().clone(),
                        original.provenance().clone(),
                    )
                    .unwrap()
                })
            })
            .collect();
        let default_evidence: Vec<_> = evidence
            .iter()
            .zip(&defaults)
            .map(|(supplied, default)| match (supplied, default) {
                (Some(supplied), Some(default_features)) => Some(
                    conduit_speech::global_intent_realization::GlobalSegmentPreparation {
                        context: supplied.context,
                        observed_features: supplied.observed_features,
                        default_features,
                    },
                ),
                _ => None,
            })
            .collect();
        let text = LanguageText::new(
            LanguageTextId::new("choice/text".into()).unwrap(),
            intent.language().clone(),
            LanguageTextRevisionId::new("choice/text-revision".into()).unwrap(),
            "tata".into(),
        )
        .unwrap();
        let materials = [conduit_speech::intent_sources::IntentSourceMaterial::Text(&text); 4];
        let prepared =
            conduit_speech::sourced_global_intent::prepare_sourced_aspirated_global_intent(
                intent,
                &materials,
                inventory,
                &profile,
                boundaries,
                &default_rules,
                &default_policy,
                &default_evidence,
            )
            .map_err(|reason| format!("{reason:?}"))?;
        super::super::super::write_rendered(
            output,
            if enabled {
                "feature-default-aspiration-on-tata"
            } else {
                "feature-default-aspiration-off-tata"
            },
            prepared
                .renderer()
                .map_err(|reason| format!("{reason:?}"))?,
        )?;
    }
    Ok(())
}
