//! All supported contextual obligations for one original declared allophone.
//! Context comparison is distinct from rule-status policy, choice and commitment.
use crate::{
    allophone_context::{
        compare_allophone_neighbors, compare_allophone_scalar_context, AllophoneNeighborContext,
        AllophoneScalarContext, ScalarContextObservation,
    },
    context_match::ContextComparisonRefusal,
    declared_realization::{DeclaredIntentRealization, PhoneDeclaration},
    generated,
    neighbor_match::NeighborComparisonRefusal,
    occurrence_context::{
        resolve_intent_occurrence_context, IntentOccurrenceContext, OccurrenceContextRefusal,
    },
    rule_conditions::{compare_conditions, ConditionRefusal, ConditionsComparison},
    semantic::*,
};

pub struct ExplicitAllophoneContext<'a> {
    pub syllable_position: &'a SpeechSyllablePositionSpecification,
    pub prosodic_context: &'a SpeechProsodicContextSpecification,
    pub careful_style: &'a SpeechCarefulStyleSpecification,
}
#[derive(Debug)]
pub enum DeclaredContextRefusal {
    MissingDeclaration,
    NotAllophone,
    Occurrence(OccurrenceContextRefusal),
    Scalar(ContextComparisonRefusal),
    Neighbor(NeighborComparisonRefusal),
    UnsupportedNeighbor { before: bool },
    Conditions(ConditionRefusal),
    CompiledPlot,
}
pub struct DeclaredAllophoneContext<'a> {
    declared: &'a DeclaredIntentRealization<'a>,
    index: usize,
    occurrence: IntentOccurrenceContext<'a>,
    scalar: AllophoneScalarContext<'a>,
    neighbors: AllophoneNeighborContext<'a>,
    conditions: ConditionsComparison<'a>,
    decision: SpeechContextDecision,
}
impl<'a> DeclaredAllophoneContext<'a> {
    pub fn declared(&self) -> &'a DeclaredIntentRealization<'a> {
        self.declared
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn occurrence(&self) -> &IntentOccurrenceContext<'a> {
        &self.occurrence
    }
    pub fn scalar(&self) -> &AllophoneScalarContext<'a> {
        &self.scalar
    }
    pub fn neighbors(&self) -> &AllophoneNeighborContext<'a> {
        &self.neighbors
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
) -> Result<SpeechContextDecision, DeclaredContextRefusal> {
    Ok(match value {
        SpeechNeighborDecision::Matched => SpeechContextDecision::Matched,
        SpeechNeighborDecision::Mismatched => SpeechContextDecision::Mismatched,
        SpeechNeighborDecision::RequirementUnresolved => {
            SpeechContextDecision::RequirementUnresolved
        }
        SpeechNeighborDecision::ObservationUnresolved => {
            SpeechContextDecision::ObservationUnresolved
        }
        SpeechNeighborDecision::UnsupportedMatcher => {
            return Err(DeclaredContextRefusal::UnsupportedNeighbor { before })
        }
    })
}
pub fn compare_declared_allophone_context<'a>(
    declared: &'a DeclaredIntentRealization<'a>,
    index: usize,
    explicit: ExplicitAllophoneContext<'a>,
) -> Result<DeclaredAllophoneContext<'a>, DeclaredContextRefusal> {
    let receipt = declared
        .declarations()
        .get(index)
        .ok_or(DeclaredContextRefusal::MissingDeclaration)?;
    let PhoneDeclaration::Allophone { declaration, .. } = receipt.declaration() else {
        return Err(DeclaredContextRefusal::NotAllophone);
    };
    let occurrence =
        resolve_intent_occurrence_context(declared.source().intent(), declared.source().event())
            .map_err(DeclaredContextRefusal::Occurrence)?;
    let scalar = compare_allophone_scalar_context(
        declaration,
        ScalarContextObservation {
            stress: occurrence.segment().stress(),
            word_position: occurrence.segment().word_position(),
            syllable_position: explicit.syllable_position,
            prosodic_context: explicit.prosodic_context,
        },
    )
    .map_err(DeclaredContextRefusal::Scalar)?;
    let neighbors = compare_allophone_neighbors(
        declaration,
        occurrence.before().observation(),
        occurrence.after().observation(),
    )
    .map_err(DeclaredContextRefusal::Neighbor)?;
    let conditions = compare_conditions(
        declaration.conditions(),
        occurrence.condition_evidence(explicit.careful_style),
    )
    .map_err(DeclaredContextRefusal::Conditions)?;
    let decisions = [
        *scalar.stress().decision(),
        *scalar.word_position().decision(),
        *scalar.syllable_position().decision(),
        *scalar.prosodic_context().decision(),
        neighbor_decision(neighbors.before().decision(), true)?,
        neighbor_decision(neighbors.after().decision(), false)?,
        *conditions.decision(),
    ];
    let mut accumulated = generated::SpeechContextDecision::matched;
    for decision in decisions {
        let right = match decision {
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
        .ok_or(DeclaredContextRefusal::CompiledPlot)?;
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
    Ok(DeclaredAllophoneContext {
        declared,
        index,
        occurrence,
        scalar,
        neighbors,
        conditions,
        decision,
    })
}
