//! The installed owner seals a presentation route independently of its
//! workload Wake. A lulled Body remains able to disclose its current actions.

use super::Owner;
use conduit_presentation::{
    AdmittedMaskPlotRoutes, LocalOwnerMaskRouteSeal, MaskShow, Presentation,
};

impl Owner {
    pub(crate) fn seal_attached_terminal_route(
        &self,
    ) -> Result<(Presentation, LocalOwnerMaskRouteSeal), String> {
        let face = self.local_face_snapshot()?;
        let planned = self.host.prepare_terminal_mask_execution()?;
        let seal = LocalOwnerMaskRouteSeal::seal_lulled(
            &self.session,
            &face,
            self.host.advertisement(),
            planned.planned_mask(),
        )
        .map_err(|error| format!("seal local owner terminal route: {error:?}"))?;
        Ok((face, seal))
    }

    pub(crate) fn validate_attached_terminal_route(
        &self,
        seal: &LocalOwnerMaskRouteSeal,
        show: &MaskShow,
    ) -> Result<(), String> {
        let face = self.local_face_snapshot()?;
        seal.validate_available_show(&self.session, &face, self.host.advertisement(), show)
            .map_err(|error| format!("stale local owner terminal route: {error:?}"))
    }

    pub(crate) fn admit_attached_terminal_show(
        &self,
        seal: &LocalOwnerMaskRouteSeal,
        show: &MaskShow,
    ) -> Result<AdmittedMaskPlotRoutes, String> {
        let face = self.local_face_snapshot()?;
        AdmittedMaskPlotRoutes::from_local_owner_show(
            seal,
            &self.session,
            &face,
            self.host.advertisement(),
            show,
        )
        .map_err(|error| format!("admit attached terminal Mask route: {error:?}"))
    }
}
