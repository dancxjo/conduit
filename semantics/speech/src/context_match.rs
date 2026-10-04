//! Borrowed scalar environment comparisons. The checked Plot owns matching
//! policy; projection retains every original specification and confidence.
use crate::{generated, semantic::*};

pub struct ContextComparison<'a, T> {
    requirement: &'a T,
    observation: &'a T,
    decision: SpeechContextDecision,
}
impl<'a, T> ContextComparison<'a, T> {
    pub fn requirement(&self) -> &'a T {
        self.requirement
    }
    pub fn observation(&self) -> &'a T {
        self.observation
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}
#[derive(Debug)]
pub enum ContextComparisonRefusal {
    CompiledPlot,
}

macro_rules! comparison {
    ($function:ident, $spec:ident) => {
        pub fn $function<'a>(
            requirement: &'a $spec,
            observation: &'a $spec,
        ) -> Result<ContextComparison<'a, $spec>, ContextComparisonRefusal> {
            use generated::SpeechSpecificationState as S;
            let state = |value: &$spec| match value {
                $spec::Known(_) => S::known,
                $spec::Unknown => S::unknown,
                $spec::Unspecified => S::unspecified,
                $spec::NotApplicable => S::not_applicable,
                $spec::Variable(_) => S::variable,
                $spec::Gradient(_) => S::gradient,
            };
            // Unit enum tags are a mechanical representation within this one
            // specification domain. Non-Known slots carry an ignored zero.
            let value_tag = |value: &$spec| match value {
                $spec::Known(value) => *value as i32,
                _ => 0,
            };
            let result =
                generated::speech_context_compare(generated::SpeechContextComparisonInput {
                    requirement: state(requirement),
                    observation: state(observation),
                    requirement_value: value_tag(requirement),
                    observation_value: value_tag(observation),
                })
                .ok_or(ContextComparisonRefusal::CompiledPlot)?;
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
            Ok(ContextComparison {
                requirement,
                observation,
                decision,
            })
        }
    };
}
comparison!(compare_stress, StressSpecification);
comparison!(compare_word_position, SpeechPositionSpecification);
comparison!(
    compare_syllable_position,
    SpeechSyllablePositionSpecification
);
comparison!(compare_prosodic_context, SpeechProsodicContextSpecification);
