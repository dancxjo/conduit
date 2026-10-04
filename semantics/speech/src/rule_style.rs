//! Explicit style-condition evidence. Prosodic labels never imply the style
//! option, and missing/variable/gradient settings do not become defaults.
use crate::{generated, semantic::*};
#[derive(Debug)]
pub enum StyleConditionRefusal {
    WrongCondition,
    CompiledPlot,
}
pub struct StyleConditionComparison<'a> {
    condition: &'a SpeechRuleCondition,
    observation: &'a SpeechCarefulStyleSpecification,
    decision: SpeechContextDecision,
}
impl<'a> StyleConditionComparison<'a> {
    pub fn condition(&self) -> &'a SpeechRuleCondition {
        self.condition
    }
    pub fn observation(&self) -> &'a SpeechCarefulStyleSpecification {
        self.observation
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}
pub fn compare_not_careful_style<'a>(
    condition: &'a SpeechRuleCondition,
    observation: &'a SpeechCarefulStyleSpecification,
) -> Result<StyleConditionComparison<'a>, StyleConditionRefusal> {
    if !matches!(condition, SpeechRuleCondition::NotCarefulStyle) {
        return Err(StyleConditionRefusal::WrongCondition);
    }
    use generated::SpeechSpecificationState as S;
    let state = match observation {
        SpeechCarefulStyleSpecification::Known(_) => S::known,
        SpeechCarefulStyleSpecification::Unknown => S::unknown,
        SpeechCarefulStyleSpecification::Unspecified => S::unspecified,
        SpeechCarefulStyleSpecification::NotApplicable => S::not_applicable,
        SpeechCarefulStyleSpecification::Variable(_) => S::variable,
        SpeechCarefulStyleSpecification::Gradient(_) => S::gradient,
    };
    // Ignored non-Known filler; the original specification remains borrowed.
    let careful = match observation {
        SpeechCarefulStyleSpecification::Known(value) => *value,
        _ => false,
    };
    let result = generated::speech_not_careful_style(generated::SpeechStyleConditionInput {
        observation: state,
        careful,
    })
    .ok_or(StyleConditionRefusal::CompiledPlot)?;
    let decision = match result {
        generated::SpeechContextDecision::matched => SpeechContextDecision::Matched,
        generated::SpeechContextDecision::mismatched => SpeechContextDecision::Mismatched,
        generated::SpeechContextDecision::observation_unresolved => {
            SpeechContextDecision::ObservationUnresolved
        }
        generated::SpeechContextDecision::requirement_unresolved => {
            SpeechContextDecision::RequirementUnresolved
        }
    };
    Ok(StyleConditionComparison {
        condition,
        observation,
        decision,
    })
}
