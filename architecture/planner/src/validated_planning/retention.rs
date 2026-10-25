//! Retire indexed configuration only after its state admission facts are captured.
//! Errors remain deferred to the original state-admission phase, preserving refusal order.
use super::*;
pub(crate) struct ConfigurationRetention {
    deferred: bool,
    pub(crate) indices: BTreeMap<PlacementId, usize>,
    states: BTreeMap<PlacementId, Result<Option<PlannedStateBoundary>, PlannerError>>,
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
            self.states
                .insert(gear.placement_id.clone(), planned_keep_state(gear));
            gear.configuration = Vec::new();
        }
    }
    pub(super) fn state(
        &mut self,
        gear: &PlannedGear,
    ) -> Result<Option<PlannedStateBoundary>, PlannerError> {
        if self.deferred {
            self.states
                .remove(&gear.placement_id)
                .expect("one captured state result per placement")
        } else {
            planned_keep_state(gear)
        }
    }
}

#[cfg(test)]
mod tests;
