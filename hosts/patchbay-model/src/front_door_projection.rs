//! Build one immutable portable projection from the living front-door session.

use crate::{
    LocalFrontDoor, LocalFrontDoorProjection, PartsView, PatchbayGraph, PatchbayPresentation,
};

impl LocalFrontDoor {
    pub fn project(&self) -> Result<LocalFrontDoorProjection, String> {
        let parts = PartsView::project(
            &self.body,
            &self.membership,
            &self.candidates,
            &self.here,
            self.plan.as_ref().map(|document| &document.exact),
            self.active_play.as_ref(),
            self.wake.is_some(),
        )
        .map_err(|error| format!("{error:?}"))?;
        let snapshot = self.topology.snapshot(
            self.model.startup_snapshot(),
            &self.candidates,
            &self.membership,
        );
        let topology = conduit_observatory::build_report(&snapshot)?;
        let graph = PatchbayGraph::from_expanded(
            &self
                .editor
                .expand_form(&self.form_name)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let projection = PatchbayPresentation::new(
            self.revision,
            self.editor.view(),
            self.plan.clone(),
            self.play.clone(),
            Some(topology),
            Vec::new(),
        )
        .map_err(|error| error.to_string())?
        .with_graph(graph)
        .map_err(|error| error.to_string())?;
        let presentation = match &self.wake {
            Some(wake) => projection
                .to_portable_front_door(&self.body, wake, &parts)
                .map_err(|error| error.to_string())?,
            None => projection
                .to_portable_lulled_front_door(&self.body, &parts)
                .map_err(|error| error.to_string())?,
        };
        let navigation = crate::PatchbayNavigationProjection::for_embodied(&presentation)?;
        Ok(LocalFrontDoorProjection {
            presentation,
            navigation,
            parts,
        })
    }
}
