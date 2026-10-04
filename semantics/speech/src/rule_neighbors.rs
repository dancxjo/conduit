//! Previous/next matcher conditions retain the original rule and exact neighbor
//! comparison. Callers must establish occurrence adjacency separately.
use crate::{
    neighbor_match::{
        compare_neighbor, NeighborComparison, NeighborComparisonRefusal, NeighborObservation,
    },
    semantic::SpeechRuleCondition,
};
#[derive(Debug)]
pub enum NeighborConditionRefusal {
    WrongCondition,
    Neighbor(NeighborComparisonRefusal),
}
pub struct NeighborConditionComparison<'a> {
    condition: &'a SpeechRuleCondition,
    comparison: NeighborComparison<'a>,
}
impl<'a> NeighborConditionComparison<'a> {
    pub fn condition(&self) -> &'a SpeechRuleCondition {
        self.condition
    }
    pub fn comparison(&self) -> &NeighborComparison<'a> {
        &self.comparison
    }
}
pub fn compare_neighbor_condition<'a>(
    condition: &'a SpeechRuleCondition,
    before: NeighborObservation<'a>,
    after: NeighborObservation<'a>,
) -> Result<NeighborConditionComparison<'a>, NeighborConditionRefusal> {
    let (requirement, observation) = match condition {
        SpeechRuleCondition::PreviousMatches(requirement) => (requirement, before),
        SpeechRuleCondition::NextMatches(requirement) => (requirement, after),
        _ => return Err(NeighborConditionRefusal::WrongCondition),
    };
    Ok(NeighborConditionComparison {
        condition,
        comparison: compare_neighbor(requirement, observation)
            .map_err(NeighborConditionRefusal::Neighbor)?,
    })
}
