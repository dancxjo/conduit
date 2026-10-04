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
            original.status().clone(),
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
    }
    Ok(())
}
