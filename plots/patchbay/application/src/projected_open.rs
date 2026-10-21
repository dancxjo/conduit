//! Admission of an already projected, immutable graph; no planner serialization.
use super::*;
impl PatchbayApplicationPort {
    pub fn open_graph(
        graph: PatchbayGraph,
        plan_id: PlanId,
        body_plan_id: PlanId,
    ) -> Result<Self, PatchbayApplicationRefusal> {
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
        Self::from_projected_graph(graph, plan_id, body_plan_id)
    }

    // Both entrances share this exact controller initialization. The original
    // Expanded Plot entrance already checked its projection; do not add stricter
    // projected-DTO bounds to that existing public behavior.
    pub(super) fn from_projected_graph(
        graph: PatchbayGraph,
        plan_id: PlanId,
        body_plan_id: PlanId,
    ) -> Result<Self, PatchbayApplicationRefusal> {
        let checked_plot_id = String::from(graph.checked_plot_id.as_str());
        let title = graph.plot_name.clone();
        let entry = PatchbayActivePlot {
            graph,
            title,
            checked_plot_id,
            plan_id,
            state: PatchbayActivePlotState::Playing,
            focused: true,
        };
        let mut port = Self::open_active(vec![entry], body_plan_id, None)?;
        port.selected_plot = Some(0);
        Ok(port)
    }
}
