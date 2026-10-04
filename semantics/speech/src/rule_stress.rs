//! Original singleton/set stress conditions with explicit immediate-neighbor
//! evidence. No stress, adjacency or boundary is inferred from token position.
use crate::{generated, semantic::*};
#[derive(Clone, Copy)]
pub enum StressObservation<'a> {
    Absent,
    Unknown,
    Boundary(&'a SpeechBoundarySpecification),
    Segment(&'a StressSpecification),
}
#[derive(Debug)]
pub enum StressConditionRefusal {
    WrongCondition,
    CompiledPlot,
}
pub struct StressConditionComparison<'a> {
    condition: &'a SpeechRuleCondition,
    observation: StressObservation<'a>,
    decision: SpeechContextDecision,
}
impl<'a> StressConditionComparison<'a> {
    pub fn condition(&self) -> &'a SpeechRuleCondition {
        self.condition
    }
    pub fn observation(&self) -> &StressObservation<'a> {
        &self.observation
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}
pub fn compare_stress_condition<'a>(
    condition: &'a SpeechRuleCondition,
    before: StressObservation<'a>,
    after: StressObservation<'a>,
) -> Result<StressConditionComparison<'a>, StressConditionRefusal> {
    let (stresses, observation) = match condition {
        SpeechRuleCondition::PreviousStress(value) => (core::slice::from_ref(value), before),
        SpeechRuleCondition::NextStress(value) => (core::slice::from_ref(value), after),
        SpeechRuleCondition::PreviousStressIn(values) => (values.as_slice(), before),
        SpeechRuleCondition::NextStressIn(values) => (values.as_slice(), after),
        _ => return Err(StressConditionRefusal::WrongCondition),
    };
    let mut allowed = [false; 4];
    for value in stresses {
        allowed[*value as usize] = true;
    }
    use generated::{SpeechNeighborPresence as P, SpeechSpecificationState as S};
    let (presence, state, stress) = match observation {
        StressObservation::Absent => (P::absent, S::unknown, 0),
        StressObservation::Unknown => (P::unknown, S::unknown, 0),
        StressObservation::Boundary(_) => (P::boundary, S::unknown, 0),
        StressObservation::Segment(value) => {
            let state = match value {
                StressSpecification::Known(_) => S::known,
                StressSpecification::Unknown => S::unknown,
                StressSpecification::Unspecified => S::unspecified,
                StressSpecification::NotApplicable => S::not_applicable,
                StressSpecification::Variable(_) => S::variable,
                StressSpecification::Gradient(_) => S::gradient,
            };
            let tag = match value {
                StressSpecification::Known(value) => *value as i32,
                _ => 0,
            };
            (P::segment, state, tag)
        }
    };
    let result = generated::speech_stress_condition(generated::SpeechStressConditionInput {
        presence,
        observation: state,
        stress,
        allow0: allowed[0],
        allow1: allowed[1],
        allow2: allowed[2],
        allow3: allowed[3],
    })
    .ok_or(StressConditionRefusal::CompiledPlot)?;
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
    Ok(StressConditionComparison {
        condition,
        observation,
        decision,
    })
}
