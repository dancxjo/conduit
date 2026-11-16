//! Closed standard graphical admission for each original native viewer lane.
//! Generic component/speech/browser factories keep separate qualification; no
//! parser, general planner, new Host or effect executor is introduced here.
use super::super::{face_wardrobe::OwnerFaceWardrobe, Owner};
use super::*;
use conduit_core::*;
use conduit_face::{CurrentOwnerFaceRoute, PlannedMaskPlot};
use std::cell::RefCell;
#[path = "staged/backing.rs"]
mod prepared_backing;
use prepared_backing::Backing;
#[path = "staged/show.rs"]
mod show;

struct StagedView {
    route: NativeMaskRoute,
    wardrobe: OwnerFaceWardrobe,
    exported: RemoteOwnerMaskRouteSeal,
}
impl Owner {
    fn prepare_canonical_native_template(&mut self) -> Result<(), String> {
        let memory = self.host.root_preparation()?.clone();
        if let Some(template) = self.native_template() {
            return memory.verify_prepared(template);
        }
        let receipt =
            conduit_conduitos_mask_offer::canonical_native_graphical_template_preparation();
        // SAFETY: the generated typed DATA constructor's exact receipt was
        // qualified against normal Make output; no parser/codec/provider runs.
        let template = unsafe {
            memory.prepare_value(
                receipt.preparation_bytes(),
                receipt.retained_bytes(),
                || Ok(receipt.construct()),
            )
        }?;
        self.install_native_template(template)
    }

    #[allow(clippy::too_many_arguments)] // Exact original three directional Lines.
    pub(in crate::durable_host::owner::controller) fn select_image_native_view(
        &mut self,
        session: &LinkBindingId,
        face_line: &LineOffer,
        return_line: &LineOffer,
        interaction_line: &LineOffer,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        self.select_native_viewer_session(session)?;
        self.inspect_current_member_carrier(session)?;
        // Mixing additional presentation routes requires its own closed
        // factory/wardrobe receipt. No8-route capacity is treated as a grant.
        if self.pending_browser.is_some()
            || self.attached_terminal_route.is_some()
            || self.direct_spoken_route.is_some()
            || self.llm_spoken_route.is_some()
        {
            return Err(
                "standard concurrent native view does not qualify mixed presentation routes".into(),
            );
        }
        self.prepare_canonical_native_template()?;
        // This existing projection is caller-owned input, not a claim that the
        // native lane receipt funds the general Source/Face producer or transport.
        let face = self.local_face_snapshot()?;
        let memory = self.host.root_preparation()?.clone();
        let facts = self.native_carrier_view(session)?;
        let credential = facts.credential;
        let target = facts.offer;
        let deadline = facts.deadline;
        let owner = self.host.advertisement();
        if !crate::durable_host::routed_join::native_lines::matches_standard_graphical_scope(
            owner,
            target,
            credential.credential_id.as_str(),
            session.as_str(),
            [face_line, return_line, interaction_line],
        ) {
            return Err(
                "member carrier Lines differ from original standard session profile".into(),
            );
        }
        let template = facts.template;
        let planned = super::graphical_template::instantiate(
            &memory,
            template,
            target,
            owner,
            face_line,
            return_line,
            interaction_line,
        )?;
        memory.verify_prepared(&planned)?;
        let bound = Backing::checked(
            credential,
            owner,
            target,
            &self.session.evidence().body,
            &face,
            planned.value(),
            [face_line, return_line, interaction_line],
        )
        .ok_or("native route/wardrobe closed backing refused")?;
        let evidence = &self.session;
        let pin = facts.pin;
        let expires = facts.expires;
        // SAFETY: Backing names actual retained route/one-child wardrobe/export
        // descendants, ordinary verification/reconciliation clones, codec fixed
        // stores and enforced output slots. Inputs and prepared target Plan stay
        // independently funded. Keeping the peak until retirement also funds
        // bounded serial validation/reconciliation; it is not whole Root/RSS.
        let prepared = unsafe {
            memory.prepare_value(bound.peak, bound.peak, || {
                let exported = RemoteOwnerMaskRouteSeal::seal_current_with_interaction(
                    evidence,
                    &face,
                    owner,
                    target,
                    planned.value(),
                    face_line,
                    return_line,
                    interaction_line,
                )
                .map_err(|error| format!("native-mask-route-refused:{error:?}"))?;
                let route = NativeMaskRoute {
                    component_selection: None,
                    credential: credential.clone(),
                    host_offer: pin.clone(),
                    face_line: face_line.clone(),
                    return_line: return_line.clone(),
                    interaction_line: Some(interaction_line.clone()),
                    read_only: false,
                    seal: exported.clone(),
                    acknowledged_show: None,
                    expires_at_millis: expires,
                    carrier_presence_deadline: Some(deadline),
                    show_memory: None,
                    memory: None,
                };
                let current = [CurrentOwnerFaceRoute::Remote {
                    seal: &route.seal,
                    owner_offer: owner,
                    mask_host_offer: target,
                    face_line: &route.face_line,
                    return_line: &route.return_line,
                    interaction_line: route.interaction_line.as_ref(),
                }];
                let plot = route.seal.planned_mask.mask.plot_identity.clone();
                let wardrobe = OwnerFaceWardrobe::seal(
                    evidence,
                    &face,
                    &current,
                    vec![plot.clone()],
                    vec![plot],
                )
                .map_err(|error| format!("native wardrobe:{error:?}"))?;
                let selected = wardrobe
                    .control()
                    .selected
                    .as_ref()
                    .ok_or("native wardrobe selected no child")?;
                if selected.route_id.strip_prefix("route/") != Some(exported.route_plan_id.as_str())
                {
                    return Err("native wardrobe selected another original child".into());
                }
                Ok(RefCell::new(Some(StagedView {
                    route,
                    wardrobe,
                    exported,
                })))
            })
        }?;
        let original = prepared.retain_memory();
        let StagedView {
            mut route,
            mut wardrobe,
            exported,
        } = prepared
            .value()
            .borrow_mut()
            .take()
            .ok_or("prepared native view absent")?;
        route.memory = Some(original.clone());
        wardrobe.retain_native_memory(original);
        // Publication is one actor turn AFTER all allocation/admission succeeds.
        self.bind_original_native_route(session, &exported.route_plan_id)?;
        self.pending_native_mask = Some(route);
        self.face_wardrobe = Some(wardrobe);
        Ok(exported)
    }
}
