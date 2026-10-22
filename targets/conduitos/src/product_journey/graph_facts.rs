//! Read-only rendering query under current Root Body/Host/Boot custody.
use super::*;
use conduitos_application_core::graph_facts::{Contribution, Scope};
impl ProductJourney {
    pub(crate) fn graph_face_facts(
        &self,
        graph: &patchbay_graph::PatchbayGraph,
        plot_present: bool,
    ) -> Result<Contribution, JourneyError> {
        let projection = self.projection();
        let body = projection.body_id.ok_or(JourneyError::BodyAbsent)?;
        let scope = Scope::new(
            projection.host_id,
            projection.boot_id,
            body,
            projection.revision,
            graph,
            plot_present,
        )
        .map_err(|_| JourneyError::Kernel)?;
        if let Some(kernel) = self.kernel.as_ref() {
            kernel
                .project_graph(graph, scope)
                .map_err(JourneyError::Play)
        } else {
            crate::protected_application::project_graph(graph, scope)
                .map_err(|_| JourneyError::Kernel)
        }
    }
}
