//! Original previous/next feature conditions against explicit neighbor facts.
//! No token/definition fallback, normalization or occurrence inference.
use crate::{
    admission::{validate_feature_bundle, LocalSemanticRefusal},
    feature_match::{compare_known_feature_value, FeatureComparisonRefusal},
    generated,
    neighbor_match::NeighborObservation,
    semantic::*,
};
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum FeatureConditionRefusal {
    WrongCondition,
    Bundle(LocalSemanticRefusal),
    Key(NativeBindingRefusal),
    Feature(FeatureComparisonRefusal),
    CompiledPlot,
}
pub struct FeatureConditionComparison<'a> {
    condition: &'a SpeechRuleCondition,
    observation: NeighborObservation<'a>,
    feature: Option<&'a SpeechFeature>,
    checked_key: Option<SpeechFeatureIdentityMatch>,
    decision: SpeechContextDecision,
}
impl<'a> FeatureConditionComparison<'a> {
    pub fn condition(&self) -> &'a SpeechRuleCondition {
        self.condition
    }
    pub fn observation(&self) -> &NeighborObservation<'a> {
        &self.observation
    }
    pub fn feature(&self) -> Option<&'a SpeechFeature> {
        self.feature
    }
    pub fn checked_key(&self) -> Option<&SpeechFeatureIdentityMatch> {
        self.checked_key.as_ref()
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}
pub fn compare_feature_condition<'a>(
    condition: &'a SpeechRuleCondition,
    before: NeighborObservation<'a>,
    after: NeighborObservation<'a>,
) -> Result<FeatureConditionComparison<'a>, FeatureConditionRefusal> {
    let (required_identity, required_value, observation) = match condition {
        SpeechRuleCondition::PreviousHasFeature(value) => (value.identity(), value.value(), before),
        SpeechRuleCondition::NextHasFeature(value) => (value.identity(), value.value(), after),
        _ => return Err(FeatureConditionRefusal::WrongCondition),
    };
    use generated::{SpeechContextDecision as D, SpeechNeighborPresence as P};
    let presence = match observation {
        NeighborObservation::Absent => P::absent,
        NeighborObservation::Unknown => P::unknown,
        NeighborObservation::Boundary(_) => P::boundary,
        NeighborObservation::Segment { .. } => P::segment,
    };
    let bundle = match observation {
        NeighborObservation::Segment { features, .. } => features,
        _ => None,
    };
    if let Some(bundle) = bundle {
        validate_feature_bundle(bundle).map_err(FeatureConditionRefusal::Bundle)?;
    }
    let feature = bundle.and_then(|bundle| {
        bundle
            .get()
            .as_slice()
            .iter()
            .find(|feature| feature.identity() == required_identity)
    });
    let checked_key = feature
        .map(|feature| {
            SpeechFeatureIdentityMatch::new(feature.identity().clone(), required_identity.clone())
        })
        .transpose()
        .map_err(FeatureConditionRefusal::Key)?;
    let compared =
        compare_known_feature_value(required_value, feature.map(SpeechFeature::specification))
            .map_err(FeatureConditionRefusal::Feature)?;
    let projected = match compared {
        SpeechContextDecision::Matched => D::matched,
        SpeechContextDecision::Mismatched => D::mismatched,
        SpeechContextDecision::RequirementUnresolved => D::requirement_unresolved,
        SpeechContextDecision::ObservationUnresolved => D::observation_unresolved,
    };
    let result = generated::speech_feature_condition(generated::SpeechFeatureConditionInput {
        presence,
        feature: projected,
    })
    .ok_or(FeatureConditionRefusal::CompiledPlot)?;
    let decision = match result {
        D::matched => SpeechContextDecision::Matched,
        D::mismatched => SpeechContextDecision::Mismatched,
        D::requirement_unresolved => SpeechContextDecision::RequirementUnresolved,
        D::observation_unresolved => SpeechContextDecision::ObservationUnresolved,
    };
    Ok(FeatureConditionComparison {
        condition,
        observation,
        feature,
        checked_key,
        decision,
    })
}
