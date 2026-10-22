//! Actual selected Mask Plan and acknowledged Show; no synthetic availability.
use super::*;
impl MaskControlSession {
    pub fn install_initial_selected_graphical(
        &mut self,
        planned: PlannedMaskPlot,
        planning: &mut BodyPlanningSession,
    ) -> Result<(), MaskControlError> {
        if planning.wake().lifecycle != WakeLifecycle::AwaitingPlan
            || !planning.current_plan().mask_topologies.is_empty()
            || planned.plan.fragments.len() != 1
        {
            return Err(MaskControlError::StaleRequest);
        }
        let mut candidate = self.clone();
        candidate.graphical = None;
        candidate.selected_graphical = Some((planned, None));
        let topology = candidate.body_topology(MaskTopologyMode::Graphical)?;
        let plots = planning.current_plan().plots.clone();
        let mut next = planning.clone();
        next.replace_proposal_with_masks(plots, vec![topology])
            .map_err(MaskControlError::BodyPlanning)?;
        *self = candidate;
        *planning = next;
        Ok(())
    }
    pub fn selected_graphical_pending(&self) -> bool {
        self.selected_graphical
            .as_ref()
            .is_some_and(|(_, show)| show.is_none())
    }
    pub fn retire_selected_graphical_show(&mut self) {
        if let Some((_, current)) = &mut self.selected_graphical {
            *current = None;
        }
    }
    pub fn acknowledge_selected_graphical(
        &mut self,
        show: &MaskShow,
    ) -> Result<(), MaskControlError> {
        let Some((planned, current)) = &mut self.selected_graphical else {
            return Err(MaskControlError::UnavailableMask);
        };
        show.validate(&self.face)
            .map_err(|_| MaskControlError::InvalidRealization)?;
        if show.planned_mask != *planned
            || show.show.lifecycle != conduit_face::ManifestationLifecycle::Available
        {
            return Err(MaskControlError::InvalidRealization);
        }
        if current.is_some() {
            return Err(MaskControlError::StaleRequest);
        }
        *current = Some(show.clone());
        Ok(())
    }
}
