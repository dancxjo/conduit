//! Authenticated owner report and explicit Body-lifetime Mask policy changes.
use super::{presentation_wardrobe::OwnerPresentationWardrobeError, Owner};
use conduit_core::{PlanId, PlotIdentity};
use conduit_presentation::{MaskWardrobeAction, OwnerPresentationChildRoute};
use serde_json::{json, Value};

impl Owner {
    /// The caller authenticates its carrier before reaching this method.
    /// Discovery never changes worn Masks or preference; Apply requires an
    /// exact outer Plan and wardrobe revision observed by that caller.
    pub(crate) fn owner_wardrobe_report(
        &mut self,
        expected_plan_id: Option<&PlanId>,
        basis_revision: u64,
        action: Option<MaskWardrobeAction>,
    ) -> Result<Value, String> {
        let face = self.local_face_snapshot()?;
        let local = Self::current_attached_terminal_route(
            &self.host,
            self.attached_terminal_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_presentation_routes_with_native(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            local,
            self.pending_native_mask.as_ref(),
            &self.session,
            &face,
            super::super::super::current_time_millis()?,
        );
        let wardrobe = self
            .presentation_wardrobe
            .as_mut()
            .ok_or("owner presentation wardrobe is not admitted")?;
        let admitted = wardrobe
            .plan()
            .admit_current_routes(&self.session, &face, &current)
            .map_err(|error| format!("owner wardrobe routes: {error:?}"))?;
        if let Some(action) = &action {
            if expected_plan_id != Some(&wardrobe.plan().plan_id) {
                return Err("owner-wardrobe-plan-stale".into());
            }
            // Eligibility is authored policy. A sealed route may be worn or
            // preferred while its current Line is unavailable; reconciliation
            // still refuses to select or show it until a witness returns.
            let sealed = |plot: &PlotIdentity| {
                admitted
                    .routes()
                    .iter()
                    .any(|route| route.mask_plot == *plot)
            };
            let admissible = match action {
                MaskWardrobeAction::Wear(plot) => sealed(plot),
                MaskWardrobeAction::Doff(_) => true,
                MaskWardrobeAction::Prefer(plots) => plots.iter().all(sealed),
            };
            if !admissible {
                return Err("owner-wardrobe-mask-unavailable".into());
            }
        }
        let (reconciliation, transition) = if let Some(action) = action {
            let transition = wardrobe
                .apply(&self.session, &face, &current, basis_revision, action)
                .map_err(super::presentation_wardrobe_runtime::wardrobe_error)?;
            (transition.reconciliation.clone(), Some(transition))
        } else {
            (
                wardrobe
                    .reconcile(&self.session, &face, &current)
                    .map_err(super::presentation_wardrobe_runtime::wardrobe_error)?,
                None,
            )
        };
        let show_id = match wardrobe.selected_show(&self.session, &face, &current) {
            Ok(show) => Some(show.show_id.as_str().to_owned()),
            Err(OwnerPresentationWardrobeError::ShowNotAcknowledged) => None,
            Err(error) => return Err(super::presentation_wardrobe_runtime::wardrobe_error(error)),
        };
        let route_descriptions = wardrobe
            .plan()
            .routes
            .iter()
            .map(|child| match child {
                OwnerPresentationChildRoute::Local { seal } => json!({
                    "route_id": format!("route/{}", seal.route_plan_id.as_str()),
                    "mask_name": seal.planned_mask.mask.plot_name,
                    "host_id": seal.owner_offer.host_id,
                }),
                OwnerPresentationChildRoute::Remote { seal } => json!({
                    "route_id": format!("route/{}", seal.route_plan_id.as_str()),
                    "mask_name": seal.planned_mask.mask.plot_name,
                    "host_id": seal.mask_host.host_id,
                }),
            })
            .collect::<Vec<_>>();
        let selected = wardrobe.control().selected.as_ref();
        let report = json!({
            "schema": "conduit.body/owner-mask-wardrobe@1",
            "body_id": self.session.evidence().body_id,
            "face_id": face.identity,
            "face_revision": face.revision,
            "owner_plan_id": wardrobe.plan().plan_id,
            "wardrobe_revision_decimal": wardrobe.control().scoped_wardrobe.wardrobe.revision.to_string(),
            "wardrobe": wardrobe.control().scoped_wardrobe.wardrobe,
            "admitted_routes": admitted.routes(),
            "route_descriptions": route_descriptions,
            "selected": selected,
            "show_id": show_id,
            "fresh_show_required": selected.is_some() && show_id.is_none(),
            "reconciliation": reconciliation,
            "transition": transition,
        });
        if serde_json::to_vec(&report)
            .map_err(|error| error.to_string())?
            .len()
            > 64 * 1024
        {
            return Err("owner-wardrobe-report-pressure".into());
        }
        Ok(report)
    }
}
