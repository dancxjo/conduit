//! Native equality plus Plot-owned specification-state policy. No feature
//! inference, definition fallback, numeric coercion or uncertainty collapse.
use crate::{generated, semantic::*};
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum FeatureComparisonRefusal {
    Native(NativeBindingRefusal),
    CompiledPlot,
}
pub struct FeatureComparison<'a> {
    requirement: &'a FeatureSpecification,
    observation: &'a FeatureSpecification,
    decision: SpeechContextDecision,
}
impl<'a> FeatureComparison<'a> {
    pub fn requirement(&self) -> &'a FeatureSpecification {
        self.requirement
    }
    pub fn observation(&self) -> &'a FeatureSpecification {
        self.observation
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}
fn state(value: &FeatureSpecification) -> generated::SpeechSpecificationState {
    use generated::SpeechSpecificationState as S;
    match value {
        FeatureSpecification::Known(_) => S::known,
        FeatureSpecification::Unknown => S::unknown,
        FeatureSpecification::Unspecified => S::unspecified,
        FeatureSpecification::NotApplicable => S::not_applicable,
        FeatureSpecification::Variable(_) => S::variable,
        FeatureSpecification::Gradient(_) => S::gradient,
    }
}
pub fn compare_feature<'a>(
    requirement: &'a FeatureSpecification,
    observation: &'a FeatureSpecification,
) -> Result<FeatureComparison<'a>, FeatureComparisonRefusal> {
    Ok(FeatureComparison {
        requirement,
        observation,
        decision: compare_feature_observation(requirement, Some(observation))?,
    })
}
pub(crate) fn compare_feature_observation(
    requirement: &FeatureSpecification,
    observation: Option<&FeatureSpecification>,
) -> Result<SpeechContextDecision, FeatureComparisonRefusal> {
    // The binary carrier projects the checked native equality fact, rather than
    // reproducing feature-value equality in Rust. Non-Known values are ignored.
    let observation_value = match (requirement, observation) {
        (FeatureSpecification::Known(expected), Some(FeatureSpecification::Known(actual))) => {
            match SpeechFeatureValueMatch::new(actual.clone(), expected.clone()) {
                Ok(_) => 0,
                Err(NativeBindingRefusal::ViolatedInvariant { index: 0 }) => 1,
                Err(reason) => return Err(FeatureComparisonRefusal::Native(reason)),
            }
        }
        _ => 0,
    };
    let result = generated::speech_context_compare(generated::SpeechContextComparisonInput {
        requirement: state(requirement),
        observation: observation
            .map(state)
            .unwrap_or(generated::SpeechSpecificationState::unknown),
        requirement_value: 0,
        observation_value,
    })
    .ok_or(FeatureComparisonRefusal::CompiledPlot)?;
    let decision = match result {
        generated::SpeechContextDecision::matched => SpeechContextDecision::Matched,
        generated::SpeechContextDecision::mismatched => SpeechContextDecision::Mismatched,
        generated::SpeechContextDecision::requirement_unresolved => {
            SpeechContextDecision::RequirementUnresolved
        }
        generated::SpeechContextDecision::observation_unresolved => {
            SpeechContextDecision::ObservationUnresolved
        }
    };
    Ok(decision)
}
