//! Original standalone rule context bound to one exact planned occurrence.
//! Input features, output inheritance, status policy and selection are separate.
use crate::{
    context_match::{
        compare_prosodic_context, compare_stress, compare_syllable_position, compare_word_position,
        ContextComparison, ContextComparisonRefusal,
    },
    declared_context::ExplicitAllophoneContext,
    generated,
    neighbor_match::{
        compare_neighbor_alternatives, NeighborAlternatives, NeighborComparisonRefusal,
    },
    occurrence_context::{
        resolve_intent_occurrence_context, IntentOccurrenceContext, OccurrenceContextRefusal,
    },
    rule_conditions::{compare_conditions, ConditionRefusal, ConditionsComparison},
    semantic::*,
};
#[derive(Debug)]
pub enum RuleContextField {
    Stress,
    WordPosition,
    SyllablePosition,
    ProsodicContext,
}
#[derive(Debug)]
pub enum RuleContextRefusal {
    Occurrence(OccurrenceContextRefusal),
    Scalar {
        field: RuleContextField,
        reason: ContextComparisonRefusal,
    },
    Neighbor {
        before: bool,
        reason: NeighborComparisonRefusal,
    },
    UnsupportedNeighbor {
        before: bool,
    },
    Conditions(ConditionRefusal),
    CompiledPlot,
}
pub struct AllophoneRuleContext<'a> {
    rule: &'a SpeechAllophoneRule,
    occurrence: IntentOccurrenceContext<'a>,
    stress: ContextComparison<'a, StressSpecification>,
    word_position: ContextComparison<'a, SpeechPositionSpecification>,
    syllable_position: ContextComparison<'a, SpeechSyllablePositionSpecification>,
    prosodic_context: ContextComparison<'a, SpeechProsodicContextSpecification>,
    before: NeighborAlternatives<'a>,
    after: NeighborAlternatives<'a>,
    conditions: ConditionsComparison<'a>,
    decision: SpeechContextDecision,
}
impl<'a> AllophoneRuleContext<'a> {
    pub fn rule(&self) -> &'a SpeechAllophoneRule {
        self.rule
    }
    pub fn occurrence(&self) -> &IntentOccurrenceContext<'a> {
        &self.occurrence
    }
    pub fn stress(&self) -> &ContextComparison<'a, StressSpecification> {
        &self.stress
    }
    pub fn word_position(&self) -> &ContextComparison<'a, SpeechPositionSpecification> {
        &self.word_position
    }
    pub fn syllable_position(&self) -> &ContextComparison<'a, SpeechSyllablePositionSpecification> {
        &self.syllable_position
    }
    pub fn prosodic_context(&self) -> &ContextComparison<'a, SpeechProsodicContextSpecification> {
        &self.prosodic_context
    }
    pub fn before(&self) -> &NeighborAlternatives<'a> {
        &self.before
    }
    pub fn after(&self) -> &NeighborAlternatives<'a> {
        &self.after
    }
    pub fn conditions(&self) -> &ConditionsComparison<'a> {
        &self.conditions
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}
fn neighbor_decision(
    value: &SpeechNeighborDecision,
    before: bool,
) -> Result<generated::SpeechContextDecision, RuleContextRefusal> {
    Ok(match value {
        SpeechNeighborDecision::Matched => generated::SpeechContextDecision::matched,
        SpeechNeighborDecision::Mismatched => generated::SpeechContextDecision::mismatched,
        SpeechNeighborDecision::RequirementUnresolved => {
            generated::SpeechContextDecision::requirement_unresolved
        }
        SpeechNeighborDecision::ObservationUnresolved => {
            generated::SpeechContextDecision::observation_unresolved
        }
        SpeechNeighborDecision::UnsupportedMatcher => {
            return Err(RuleContextRefusal::UnsupportedNeighbor { before })
        }
    })
}
fn projected(value: &SpeechContextDecision) -> generated::SpeechContextDecision {
    match value {
        SpeechContextDecision::Matched => generated::SpeechContextDecision::matched,
        SpeechContextDecision::Mismatched => generated::SpeechContextDecision::mismatched,
        SpeechContextDecision::RequirementUnresolved => {
            generated::SpeechContextDecision::requirement_unresolved
        }
        SpeechContextDecision::ObservationUnresolved => {
            generated::SpeechContextDecision::observation_unresolved
        }
    }
}
pub fn compare_allophone_rule_context<'a>(
    rule: &'a SpeechAllophoneRule,
    intent: &'a SpeechUtteranceIntent,
    event: usize,
    explicit: ExplicitAllophoneContext<'a>,
) -> Result<AllophoneRuleContext<'a>, RuleContextRefusal> {
    let occurrence =
        resolve_intent_occurrence_context(intent, event).map_err(RuleContextRefusal::Occurrence)?;
    let environment = rule.environment();
    let stress = compare_stress(environment.stress_context(), occurrence.segment().stress())
        .map_err(|reason| RuleContextRefusal::Scalar {
            field: RuleContextField::Stress,
            reason,
        })?;
    let word_position = compare_word_position(
        environment.word_position(),
        occurrence.segment().word_position(),
    )
    .map_err(|reason| RuleContextRefusal::Scalar {
        field: RuleContextField::WordPosition,
        reason,
    })?;
    let syllable_position =
        compare_syllable_position(environment.syllable_position(), explicit.syllable_position)
            .map_err(|reason| RuleContextRefusal::Scalar {
                field: RuleContextField::SyllablePosition,
                reason,
            })?;
    let prosodic_context =
        compare_prosodic_context(environment.prosodic_context(), explicit.prosodic_context)
            .map_err(|reason| RuleContextRefusal::Scalar {
                field: RuleContextField::ProsodicContext,
                reason,
            })?;
    let before = compare_neighbor_alternatives(
        environment.before().as_slice(),
        occurrence.before().observation(),
    )
    .map_err(|reason| RuleContextRefusal::Neighbor {
        before: true,
        reason,
    })?;
    let after = compare_neighbor_alternatives(
        environment.after().as_slice(),
        occurrence.after().observation(),
    )
    .map_err(|reason| RuleContextRefusal::Neighbor {
        before: false,
        reason,
    })?;
    let conditions = compare_conditions(
        rule.conditions(),
        occurrence.condition_evidence(explicit.careful_style),
    )
    .map_err(RuleContextRefusal::Conditions)?;
    let decisions = [
        projected(stress.decision()),
        projected(word_position.decision()),
        projected(syllable_position.decision()),
        projected(prosodic_context.decision()),
        neighbor_decision(before.decision(), true)?,
        neighbor_decision(after.decision(), false)?,
        projected(conditions.decision()),
    ];
    let mut result = generated::SpeechContextDecision::matched;
    for right in decisions {
        result = generated::speech_context_conjunction(generated::SpeechContextConjunction {
            left: result,
            right,
        })
        .ok_or(RuleContextRefusal::CompiledPlot)?;
    }
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
    Ok(AllophoneRuleContext {
        rule,
        occurrence,
        stress,
        word_position,
        syllable_position,
        prosodic_context,
        before,
        after,
        conditions,
        decision,
    })
}
