//! Admission of an already projected, immutable graph; no planner serialization.
use super::*;
impl PatchbayApplicationPort {
    pub fn open_graph(
        graph: PatchbayGraph,
        plan_id: PlanId,
        body_plan_id: PlanId,
    ) -> Result<Self, PatchbayApplicationRefusal> {
        validate_projected_graph(&graph)?;
        Self::from_projected_graph(graph, plan_id, body_plan_id)
    }

    /// Preparation only: reserve the declared finite inventory before Play.
    pub fn with_prepared_graph_capacity(
        mut self,
        maximum: usize,
    ) -> Result<Self, PatchbayApplicationRefusal> {
        if maximum < self.plots.len()
            || maximum > conduit_face::MAX_APPLICATION_ACTIONS.saturating_sub(4)
        {
            return Err(PatchbayApplicationRefusal::TooManyActivePlots);
        }
        self.plots.reserve_exact(maximum - self.plots.len());
        Ok(self)
    }

    /// Move an admitted graph into reserved preparation storage without growth.
    pub fn append_prepared_graph(
        &mut self,
        graph: PatchbayGraph,
        plan_id: PlanId,
        body_plan_id: &PlanId,
    ) -> Result<usize, PatchbayApplicationRefusal> {
        if self.plots.len() == self.plots.capacity()
            || self.plots.len() + 5 > conduit_face::MAX_APPLICATION_ACTIONS
        {
            return Err(PatchbayApplicationRefusal::TooManyActivePlots);
        }
        validate_projected_graph(&graph)?;
        if &self.body_plan_id != body_plan_id {
            return Err(PatchbayApplicationRefusal::UnknownAction);
        }
        if self.plots.iter().any(|plot| {
            plot.graph.checked_plot_id == graph.checked_plot_id && plot.plan_id == plan_id
        }) {
            return Err(PatchbayApplicationRefusal::UnknownAction);
        }
        let index = self.plots.len();
        self.plots.push(projected_entry(graph, plan_id));
        Ok(index)
    }

    /// Initial/native owner selection uses the same retained actor, not a clone.
    pub fn select_prepared_plot(&mut self, index: usize) -> Result<(), PatchbayApplicationRefusal> {
        if index >= self.plots.len() {
            return Err(PatchbayApplicationRefusal::UnknownAction);
        }
        self.selected_plot = Some(index);
        self.selected_subject = 0;
        self.edit_requested = false;
        Ok(())
    }
    pub fn selected_plot_index(&self) -> Option<usize> {
        self.selected_plot
    }
    pub fn selected_graph(&self) -> Option<&PatchbayGraph> {
        self.selected_plot
            .and_then(|index| self.plots.get(index))
            .map(|plot| &plot.graph)
    }
    pub fn selected_plan_id(&self) -> Option<&PlanId> {
        self.selected_plot
            .and_then(|index| self.plots.get(index))
            .map(|plot| &plot.plan_id)
    }

    // Both entrances share this exact controller initialization. The original
    // Expanded Plot entrance already checked its projection; do not add stricter
    // projected-DTO bounds to that existing public behavior.
    pub(super) fn from_projected_graph(
        graph: PatchbayGraph,
        plan_id: PlanId,
        body_plan_id: PlanId,
    ) -> Result<Self, PatchbayApplicationRefusal> {
        let mut entry = projected_entry(graph, plan_id);
        entry.focused = true;
        let mut port = Self::open_active(vec![entry], body_plan_id, None)?;
        port.selected_plot = Some(0);
        Ok(port)
    }
}

fn projected_entry(graph: PatchbayGraph, plan_id: PlanId) -> PatchbayActivePlot {
    PatchbayActivePlot {
        title: graph.plot_name.clone(),
        checked_plot_id: String::from(graph.checked_plot_id.as_str()),
        graph,
        plan_id,
        state: PatchbayActivePlotState::Playing,
        focused: false,
    }
}

fn validate_projected_graph(graph: &PatchbayGraph) -> Result<(), PatchbayApplicationRefusal> {
    use patchbay_graph::{
        MAX_FACE_CONTROLS, MAX_PATCHBAY_CORDS, MAX_PATCHBAY_GEARS, MAX_PATCHBAY_PORTS,
        MAX_PATCHBAY_SUBJECTS,
    };
    let port_count = graph.front_inputs.len()
        + graph.front_outputs.len()
        + graph
            .gears
            .iter()
            .map(|g| g.inputs.len() + g.outputs.len())
            .sum::<usize>()
        + graph
            .compositions
            .iter()
            .map(|g| g.inputs.len() + g.outputs.len())
            .sum::<usize>();
    let error = if graph.gears.len() > MAX_PATCHBAY_GEARS {
        Some(PatchbayGraphError::TooManyGears)
    } else if graph.cords.len() > MAX_PATCHBAY_CORDS {
        Some(PatchbayGraphError::TooManyCords)
    } else if port_count > MAX_PATCHBAY_PORTS {
        Some(PatchbayGraphError::TooManyPorts)
    } else if graph.subject_identities().count() > MAX_PATCHBAY_SUBJECTS {
        Some(PatchbayGraphError::TooManySubjects)
    } else if graph
        .gears
        .iter()
        .any(|g| g.controls.len() > MAX_FACE_CONTROLS)
    {
        Some(PatchbayGraphError::TooManyControls)
    } else {
        None
    };
    if let Some(error) = error {
        return Err(PatchbayApplicationRefusal::Graph(error));
    }
    Ok(())
}
