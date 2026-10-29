//! Retire indexed configuration only after its state admission facts are captured.
//! Errors remain deferred to the original state-admission phase, preserving refusal order.
use super::*;
pub(crate) struct ConfigurationRetention {
    deferred: bool,
    pub(crate) indices: BTreeMap<PlacementId, usize>,
    states: BTreeMap<PlacementId, Result<PlannedStateBoundary, PlannerError>>,
}
impl ConfigurationRetention {
    pub(crate) fn new(deferred: bool) -> Self {
        Self {
            deferred,
            indices: BTreeMap::new(),
            states: BTreeMap::new(),
        }
    }
    pub(super) fn placement(&mut self, index: usize, gear: &mut PlannedGear) {
        if self.deferred {
            self.indices.insert(gear.placement_id.clone(), index);
            // Absence is the common state result. Retaining an enum-sized tree
            // slot for every stateless Gear duplicates preparation bookkeeping.
            // Actual state and errors remain deferred to their original phase.
            match planned_keep_state(gear) {
                Ok(None) => {}
                Ok(Some(state)) => {
                    self.states.insert(gear.placement_id.clone(), Ok(state));
                }
                Err(error) => {
                    self.states.insert(gear.placement_id.clone(), Err(error));
                }
            }
            gear.configuration = Vec::new();
        }
    }
    pub(super) fn state(
        &mut self,
        gear: &PlannedGear,
    ) -> Result<Option<PlannedStateBoundary>, PlannerError> {
        if self.deferred {
            assert!(
                self.indices.contains_key(&gear.placement_id),
                "captured placement"
            );
            self.states.remove(&gear.placement_id).transpose()
        } else {
            planned_keep_state(gear)
        }
    }
}

#[cfg(test)]
mod tests;
