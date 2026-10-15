//! Preserve opaque checked wording while binding an exact authenticated owner route.
use super::*;
use conduit_core::{seal_plan_with_activation_entries, PlotIdentity};
use conduit_face::{PlannedMaskPlot, RemoteOwnerMaskRouteSeal};

impl PreparedDirectSpokenTemplateMask {
    /// The caller authenticates the owner and retains the actual carrier grant.
    /// This binds only its exact Plan; a serialized seal alone is not admission.
    pub fn with_remote_route(
        mut self,
        host: &StdHost,
        face: &Face,
        route: &RemoteOwnerMaskRouteSeal,
    ) -> Result<Self, String> {
        self.validate_current(host, face)?;
        validate(host, face, &self.mask.planned_mask, route)?;
        self.mask.plan = route.planned_mask.plan.clone();
        self.mask.planned_mask = route.planned_mask.clone();
        self.remote_route = Some(route.clone());
        Ok(self)
    }
}

pub(super) fn validate(
    host: &StdHost,
    face: &Face,
    canonical: &PlannedMaskPlot,
    route: &RemoteOwnerMaskRouteSeal,
) -> Result<(), String> {
    route
        .validate_mask_host_offer(host.advertisement())
        .map_err(|error| format!("remote spoken Host/Plan:{error:?}"))?;
    if route.face_id != face.identity
        || route.face_revision != face.revision
        || route.face_basis != face.basis
        || route.planned_mask.mask != canonical.mask
        || route.interaction_line.is_some()
        || !route
            .planned_mask
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.fore_ports)
            .any(|fore| fore.selected_line.is_some())
    {
        return Err(
            "remote spoken route differs from the checked Face/Mask or lacks selected Lines".into(),
        );
    }
    // Invert only external Fore selection to verify the original exact Plan.
    // This comparison Plan is never executed: execution retains every owner Line.
    let mut comparison = route.planned_mask.plan.clone();
    for fore in comparison
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.fore_ports)
    {
        fore.selected_line = None;
    }
    let comparison = seal_plan_with_activation_entries(
        PlotIdentity {
            source_document_id: comparison.source_document_id,
            checked_plot_id: comparison.checked_plot_id,
            expanded_plot_id: comparison.expanded_plot_id,
        },
        comparison.completion_policy,
        comparison.realization_backs,
        comparison.activations,
        comparison.fragments,
    );
    if comparison != canonical.plan {
        return Err("remote spoken route changed more than the checked Plan's Fore Lines".into());
    }
    Ok(())
}
