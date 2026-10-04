//! Bounded conjunction of original rule conditions and their inspection receipts.
//! Evidence is supplied explicitly; this does not establish occurrence adjacency.
use crate::{
    generated,
    neighbor_match::NeighborObservation,
    rule_features::{
        compare_feature_condition, FeatureConditionComparison, FeatureConditionRefusal,
    },
    rule_neighbors::{
        compare_neighbor_condition, NeighborConditionComparison, NeighborConditionRefusal,
    },
    rule_stress::{
        compare_stress_condition, StressConditionComparison, StressConditionRefusal,
        StressObservation,
    },
    rule_style::{compare_not_careful_style, StyleConditionComparison, StyleConditionRefusal},
    semantic::{
        SpeechCarefulStyleSpecification, SpeechContextDecision, SpeechNeighborDecision,
        SpeechRuleCondition,
    },
};
use conduit_plot::rust_binding::BoundedSequence;

#[derive(Clone, Copy)]
pub struct ConditionEvidence<'a> {
    pub before: NeighborObservation<'a>,
    pub after: NeighborObservation<'a>,
    pub before_stress: StressObservation<'a>,
    pub after_stress: StressObservation<'a>,
    pub careful_style: &'a SpeechCarefulStyleSpecification,
}

// Inline receipts keep storage finite; boxing would add allocation.
#[allow(clippy::large_enum_variant)]
pub enum ConditionReceipt<'a> {
    Neighbor(NeighborConditionComparison<'a>),
    Feature(FeatureConditionComparison<'a>),
    Stress(StressConditionComparison<'a>),
    Style(StyleConditionComparison<'a>),
}
impl<'a> ConditionReceipt<'a> {
    pub fn condition(&self) -> &'a SpeechRuleCondition {
        match self {
            Self::Neighbor(value) => value.condition(),
            Self::Feature(value) => value.condition(),
            Self::Stress(value) => value.condition(),
            Self::Style(value) => value.condition(),
        }
    }
    pub fn decision(&self) -> Result<SpeechContextDecision, ConditionRefusalReason> {
        Ok(match self {
            Self::Neighbor(value) => match value.comparison().decision() {
                SpeechNeighborDecision::Matched => SpeechContextDecision::Matched,
                SpeechNeighborDecision::Mismatched => SpeechContextDecision::Mismatched,
                SpeechNeighborDecision::RequirementUnresolved => {
                    SpeechContextDecision::RequirementUnresolved
                }
                SpeechNeighborDecision::ObservationUnresolved => {
                    SpeechContextDecision::ObservationUnresolved
                }
                // Refused before a receipt is admitted below.
                SpeechNeighborDecision::UnsupportedMatcher => {
                    return Err(ConditionRefusalReason::UnsupportedMatcher)
                }
            },
            Self::Feature(value) => *value.decision(),
            Self::Stress(value) => *value.decision(),
            Self::Style(value) => *value.decision(),
        })
    }
}
#[derive(Debug)]
pub enum ConditionRefusalReason {
    UnsupportedSyntax,
    UnsupportedMatcher,
    Neighbor(NeighborConditionRefusal),
    Feature(FeatureConditionRefusal),
    Stress(StressConditionRefusal),
    Style(StyleConditionRefusal),
    CompiledPlot,
}
#[derive(Debug)]
pub struct ConditionRefusal {
    pub index: usize,
    pub reason: ConditionRefusalReason,
}
pub struct ConditionsComparison<'a> {
    conditions: &'a BoundedSequence<SpeechRuleCondition, 8>,
    evidence: ConditionEvidence<'a>,
    receipts: [Option<ConditionReceipt<'a>>; 8],
    decision: SpeechContextDecision,
}
impl<'a> ConditionsComparison<'a> {
    pub fn conditions(&self) -> &'a BoundedSequence<SpeechRuleCondition, 8> {
        self.conditions
    }
    pub fn evidence(&self) -> &ConditionEvidence<'a> {
        &self.evidence
    }
    pub fn receipts(&self) -> impl Iterator<Item = &ConditionReceipt<'a>> {
        self.receipts.iter().flatten()
    }
    pub fn decision(&self) -> &SpeechContextDecision {
        &self.decision
    }
}
pub fn compare_conditions<'a>(
    conditions: &'a BoundedSequence<SpeechRuleCondition, 8>,
    evidence: ConditionEvidence<'a>,
) -> Result<ConditionsComparison<'a>, ConditionRefusal> {
    use ConditionRefusalReason as R;
    use SpeechRuleCondition as C;
    let mut receipts = core::array::from_fn(|_| None);
    let mut accumulated = generated::SpeechContextDecision::matched;
    for (index, condition) in conditions.as_slice().iter().enumerate() {
        let compare = || -> Result<ConditionReceipt<'a>, R> {
            Ok(match condition {
                C::PreviousMatches(_) | C::NextMatches(_) => {
                    let value =
                        compare_neighbor_condition(condition, evidence.before, evidence.after)
                            .map_err(R::Neighbor)?;
                    if matches!(
                        value.comparison().decision(),
                        SpeechNeighborDecision::UnsupportedMatcher
                    ) {
                        return Err(R::UnsupportedMatcher);
                    }
                    ConditionReceipt::Neighbor(value)
                }
                C::PreviousHasFeature(_) | C::NextHasFeature(_) => ConditionReceipt::Feature(
                    compare_feature_condition(condition, evidence.before, evidence.after)
                        .map_err(R::Feature)?,
                ),
                C::PreviousStress(_)
                | C::PreviousStressIn(_)
                | C::NextStress(_)
                | C::NextStressIn(_) => ConditionReceipt::Stress(
                    compare_stress_condition(
                        condition,
                        evidence.before_stress,
                        evidence.after_stress,
                    )
                    .map_err(R::Stress)?,
                ),
                C::NotCarefulStyle => ConditionReceipt::Style(
                    compare_not_careful_style(condition, evidence.careful_style)
                        .map_err(R::Style)?,
                ),
                C::CurrentWordHasSyntacticLink(_)
                | C::PreviousWordHasSyntacticLink(_)
                | C::NextWordHasSyntacticLink(_) => return Err(R::UnsupportedSyntax),
            })
        };
        let receipt = compare().map_err(|reason| ConditionRefusal { index, reason })?;
        let right = match receipt
            .decision()
            .map_err(|reason| ConditionRefusal { index, reason })?
        {
            SpeechContextDecision::Matched => generated::SpeechContextDecision::matched,
            SpeechContextDecision::Mismatched => generated::SpeechContextDecision::mismatched,
            SpeechContextDecision::RequirementUnresolved => {
                generated::SpeechContextDecision::requirement_unresolved
            }
            SpeechContextDecision::ObservationUnresolved => {
                generated::SpeechContextDecision::observation_unresolved
            }
        };
        accumulated = generated::speech_context_conjunction(generated::SpeechContextConjunction {
            left: accumulated,
            right,
        })
        .ok_or(ConditionRefusal {
            index,
            reason: R::CompiledPlot,
        })?;
        receipts[index] = Some(receipt);
    }
    let decision = match accumulated {
        generated::SpeechContextDecision::matched => SpeechContextDecision::Matched,
        generated::SpeechContextDecision::mismatched => SpeechContextDecision::Mismatched,
        generated::SpeechContextDecision::requirement_unresolved => {
            SpeechContextDecision::RequirementUnresolved
        }
        generated::SpeechContextDecision::observation_unresolved => {
            SpeechContextDecision::ObservationUnresolved
        }
    };
    Ok(ConditionsComparison {
        conditions,
        evidence,
        receipts,
        decision,
    })
}
