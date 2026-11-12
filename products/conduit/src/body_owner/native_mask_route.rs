//! Owner selection of a ConduitOS or attached terminal Mask from an admitted
//! native Part and its current offer. The caller supplies real directional Lines;
//! membership and matching identifiers cannot create them.

use super::{super::MemberPresentationBasis, Owner};
use conduit_body::{BodyLifecycleSession, MembershipCredential};
use conduit_core::{HostAdvertisement, LineAvailability, LineOffer};
use conduit_face::{Face, MaskShow, OwnerFaceSnapshotRequest, RemoteOwnerMaskRouteSeal};
use conduit_std_host::direct_spoken_template_mask::RemoteSpokenTemplateSelection;

#[path = "native_mask_route/graphical_plan.rs"]
mod graphical_plan;
#[path = "native_mask_route/member_basis.rs"]
mod member_basis;
#[path = "native_mask_route/member_leave.rs"]
mod member_leave;
#[path = "native_mask_route/profile.rs"]
mod profile;

enum SelectedRemoteMask<'a> {
    Native(
        Option<(
            conduit_face::ComponentMaskLook,
            conduit_face::TodoComponentRecipe,
        )>,
        Option<&'a LineOffer>,
    ),
    Terminal(&'a LineOffer),
    Spoken(&'a RemoteSpokenTemplateSelection),
}

#[derive(Clone)]
pub(super) struct NativeMaskRoute {
    component_selection: Option<(
        conduit_face::ComponentMaskLook,
        conduit_face::TodoComponentRecipe,
    )>,
    credential: MembershipCredential,
    host_offer: conduit_core::HostAdvertisement,
    face_line: LineOffer,
    return_line: LineOffer,
    interaction_line: Option<LineOffer>,
    read_only: bool,
    seal: RemoteOwnerMaskRouteSeal,
    acknowledged_show: Option<MaskShow>,
    expires_at_millis: u64,
    carrier_presence_deadline: Option<std::time::Instant>,
}

impl NativeMaskRoute {
    /// The return grant is finite. A missing Part, changed Face, expired grant,
    /// or observed unavailable Line withdraws this witness from the owner Plan.
    /// A purportedly Ready but malformed Line remains a witness so admission
    /// can refuse it instead of laundering that claim into ordinary loss.
    pub(super) fn current_witness(
        &self,
        session: &BodyLifecycleSession,
        face: &Face,
        now_millis: u64,
    ) -> Option<(
        &RemoteOwnerMaskRouteSeal,
        &HostAdvertisement,
        &LineOffer,
        &LineOffer,
        Option<&LineOffer>,
    )> {
        if self
            .carrier_presence_deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
            || now_millis >= self.expires_at_millis
            || self.seal.body_id != session.evidence().body_id
            || self.seal.workload_revision != session.evidence().body.workload_revision
            || self.seal.face_id != face.identity
            || self.seal.face_revision != face.revision
            || self.seal.face_basis != face.basis
            || self.face_line.availability.availability != LineAvailability::Ready
            || self.return_line.availability.availability != LineAvailability::Ready
            || self
                .interaction_line
                .as_ref()
                .is_some_and(|line| line.availability.availability != LineAvailability::Ready)
        {
            return None;
        }
        let current_part = session.evidence().membership.parts.iter().any(|part| {
            part.part_id == self.credential.part_id
                && part.current.as_ref().is_some_and(|host| {
                    host.host_id == self.credential.host_id
                        && host.boot_id == self.credential.boot_id
                        && host.offer_generation == self.host_offer.offer_generation
                })
        });
        current_part.then_some((
            &self.seal,
            &self.host_offer,
            &self.face_line,
            &self.return_line,
            self.interaction_line.as_ref(),
        ))
    }
}

impl Owner {
    pub(crate) fn seal_native_mask_route(
        &mut self,
        member: &MemberPresentationBasis,
        face_line: &LineOffer,
        return_line: &LineOffer,
        expires_at_millis: u64,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        self.seal_graphical_native_mask_route(
            member,
            face_line,
            return_line,
            None,
            expires_at_millis,
        )
    }
    pub(crate) fn seal_graphical_native_mask_route(
        &mut self,
        member: &MemberPresentationBasis,
        face_line: &LineOffer,
        return_line: &LineOffer,
        interaction_line: Option<&LineOffer>,
        expires_at_millis: u64,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let selection = self
            .pending_native_mask
            .as_ref()
            .filter(|route| route.credential == member.credential)
            .and_then(|route| route.component_selection);
        let previous_route = self.pending_native_mask.clone();
        let previous_wardrobe = self.face_wardrobe.clone();
        let result = self
            .seal_native_mask_route_with_selection(
                member,
                face_line,
                return_line,
                expires_at_millis,
                SelectedRemoteMask::Native(selection, interaction_line),
            )
            .and_then(|seal| {
                self.select_component_face_route(&seal)?;
                Ok(seal)
            });
        if result.is_err() {
            self.pending_native_mask = previous_route;
            self.face_wardrobe = previous_wardrobe;
        }
        result
    }

    pub(crate) fn seal_remote_terminal_mask_route(
        &mut self,
        member: &MemberPresentationBasis,
        face_line: &LineOffer,
        return_line: &LineOffer,
        interaction_line: &LineOffer,
        expires_at_millis: u64,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        // This authenticated request explicitly selects the terminal Mask.
        // Admission alone preserves an existing browser preference, which
        // cannot acknowledge a Show from this different presentation route.
        let previous_route = self.pending_native_mask.clone();
        let previous_wardrobe = self.face_wardrobe.clone();
        let result = self
            .seal_native_mask_route_with_selection(
                member,
                face_line,
                return_line,
                expires_at_millis,
                SelectedRemoteMask::Terminal(interaction_line),
            )
            .and_then(|seal| {
                self.select_component_face_route(&seal)?;
                Ok(seal)
            });
        if result.is_err() {
            self.pending_native_mask = previous_route;
            self.face_wardrobe = previous_wardrobe;
        }
        result
    }

    pub(crate) fn seal_remote_spoken_mask_route(
        &mut self,
        member: &MemberPresentationBasis,
        face_line: &LineOffer,
        return_line: &LineOffer,
        expires_at_millis: u64,
        selection: &RemoteSpokenTemplateSelection,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let previous_route = self.pending_native_mask.clone();
        let previous_wardrobe = self.face_wardrobe.clone();
        let result = self
            .seal_native_mask_route_with_selection(
                member,
                face_line,
                return_line,
                expires_at_millis,
                SelectedRemoteMask::Spoken(selection),
            )
            .and_then(|seal| {
                self.select_component_face_route(&seal)?;
                Ok(seal)
            });
        if result.is_err() {
            self.pending_native_mask = previous_route;
            self.face_wardrobe = previous_wardrobe;
        }
        result
    }

    fn seal_native_mask_route_with_selection(
        &mut self,
        member: &MemberPresentationBasis,
        face_line: &LineOffer,
        return_line: &LineOffer,
        expires_at_millis: u64,
        selection: SelectedRemoteMask<'_>,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let (component_selection, interaction_line, spoken, graphical) = match selection {
            SelectedRemoteMask::Native(component, line) => (component, line, None, true),
            SelectedRemoteMask::Terminal(line) => (None, Some(line), None, false),
            SelectedRemoteMask::Spoken(selection) => (None, None, Some(selection), false),
        };
        if super::super::super::current_time_millis()? >= expires_at_millis {
            return Err("native-mask-return-expired".into());
        }
        let credential = &member.credential;
        let native_offer = &member.host_advertisement;
        self.require_current_native_member(member)?;
        if (interaction_line.is_some() || spoken.is_some())
            && self.require_observed_member_offer(native_offer).is_err()
        {
            return Err("remote-terminal-offer-not-observed-at-admission".into());
        }
        // This Part's admission changed the owner Face. Keep the already
        // attached browser route in the same new-Face Plan only after its
        // actual carrier and Lines have been revalidated and resealed.
        self.refresh_browser_mask_route_for_current_face()?;
        let face = self.local_face_snapshot()?;
        let planned = if let Some(selection) = spoken {
            super::remote_spoken_route::plan(
                native_offer,
                self.host.advertisement(),
                &face,
                face_line,
                return_line,
                selection,
            )?
        } else if let Some(interaction_line) = interaction_line.filter(|_| !graphical) {
            super::remote_terminal_route::plan(
                native_offer,
                self.host.advertisement(),
                face_line,
                return_line,
                interaction_line,
            )?
        } else {
            graphical_plan::plan(
                native_offer,
                self.host.advertisement(),
                component_selection,
                face_line,
                return_line,
                interaction_line,
            )?
        };
        let selected_input = planned
            .plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .find(|p| p.gear_id == planned.mask.interaction_output.gear_id)
            .ok_or("native-mask-input-back-absent")?;
        let native_read_only = matches!(
            selected_input.execution_profile_id.as_str(),
            conduit_conduitos_mask_offer::READ_ONLY_INTERACTION_PROFILE
                | conduit_conduitos_mask_offer::READ_ONLY_COLLECTION_INTERACTION_PROFILE
        );
        if graphical && native_read_only && interaction_line.is_some() {
            return Err("native-read-only-mask-cannot-admit-interaction-Line".into());
        }
        if native_read_only
            && (!selected_input.resources.is_empty() || !selected_input.authority.is_empty())
        {
            return Err("native-read-only-input-has-unexpected-authority".into());
        }
        let seal = if let Some(interaction_line) = interaction_line {
            RemoteOwnerMaskRouteSeal::seal_current_with_interaction(
                &self.session,
                &face,
                self.host.advertisement(),
                native_offer,
                &planned,
                face_line,
                return_line,
                interaction_line,
            )
        } else if spoken.is_some() {
            RemoteOwnerMaskRouteSeal::seal_current_hosted_presentation(
                &self.session,
                &face,
                self.host.advertisement(),
                native_offer,
                &planned,
                face_line,
                return_line,
            )
        } else {
            RemoteOwnerMaskRouteSeal::seal_current(
                &self.session,
                &face,
                self.host.advertisement(),
                native_offer,
                &planned,
                face_line,
                return_line,
            )
        }
        .map_err(|error| format!("native-mask-route-refused:{error:?}"))?;
        let previous = self.pending_native_mask.replace(NativeMaskRoute {
            component_selection,
            credential: credential.clone(),
            host_offer: native_offer.clone(),
            face_line: face_line.clone(),
            return_line: return_line.clone(),
            interaction_line: interaction_line.cloned(),
            read_only: spoken.is_some() || native_read_only,
            seal: seal.clone(),
            acknowledged_show: None,
            expires_at_millis,
            carrier_presence_deadline: None,
        });
        if let Err(error) = self.admit_native_face_route(&seal) {
            self.pending_native_mask = previous;
            return Err(error);
        }
        Ok(seal)
    }

    pub(crate) fn select_native_component_mask(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        member: &MemberPresentationBasis,
        look: conduit_face::ComponentMaskLook,
        recipe: conduit_face::TodoComponentRecipe,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        self.validate_native_mask_show(request, show)?;
        let previous_route = self
            .pending_native_mask
            .clone()
            .ok_or("native-mask-route-not-selected")?;
        if previous_route.credential != member.credential
            || previous_route.host_offer != member.host_advertisement
        {
            return Err("native-mask-route-credential-mismatch".into());
        }
        let previous_wardrobe = self.face_wardrobe.clone();
        let result = self
            .seal_native_mask_route_with_selection(
                member,
                &previous_route.face_line,
                &previous_route.return_line,
                previous_route.expires_at_millis,
                SelectedRemoteMask::Native(
                    Some((look, recipe)),
                    previous_route.interaction_line.as_ref(),
                ),
            )
            .and_then(|route| {
                self.select_component_face_route(&route)?;
                Ok(route)
            });
        if result.is_err() {
            self.pending_native_mask = Some(previous_route);
            self.face_wardrobe = previous_wardrobe;
        }
        result
    }

    pub(crate) fn acknowledge_native_mask_show(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
    ) -> Result<(), String> {
        self.validate_native_mask_route_show(request, show)?;
        let seal = self
            .pending_native_mask
            .as_ref()
            .ok_or("native-mask-route-not-selected")?
            .seal
            .clone();
        self.acknowledge_selected_native_show(&seal, show)?;
        let route = self
            .pending_native_mask
            .as_mut()
            .ok_or("native-mask-route-not-selected")?;
        route.acknowledged_show = Some(show.clone());
        Ok(())
    }

    pub(crate) fn validate_native_mask_show(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
    ) -> Result<(), String> {
        self.validate_native_mask_route_show(request, show)?;
        let route = self
            .pending_native_mask
            .as_ref()
            .ok_or("native-mask-route-not-selected")?;
        if route.acknowledged_show.as_ref() != Some(show) {
            return Err("native-mask-show-not-acknowledged".into());
        }
        self.validate_selected_native_show(show)
    }

    pub(crate) fn validate_native_mask_interaction(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        interaction: &conduit_face::FaceInteraction,
    ) -> Result<(), String> {
        self.validate_native_mask_show(request, show)?;
        let route = self
            .pending_native_mask
            .as_ref()
            .ok_or("native-mask-route-not-selected")?;
        if route.read_only {
            return Err("native-mask-has-no-interaction-authority".into());
        }
        let line = route
            .interaction_line
            .as_ref()
            .ok_or("native-mask-has-no-original-interaction-Line")?;
        if route.seal.interaction_line.as_ref() != Some(&line.admitted_line()) {
            return Err("native-mask-interaction-Line-differs-from-original-route".into());
        }
        let bytes = serde_json::to_vec(interaction)
            .map_err(|error| format!("native-mask-interaction-encoding:{error}"))?;
        route
            .seal
            .validate_interaction_payload(bytes.len())
            .map_err(|error| format!("native-mask-interaction-refused:{error:?}"))?;
        Ok(())
    }

    fn validate_native_mask_route_show(
        &self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
    ) -> Result<(), String> {
        let route = self
            .pending_native_mask
            .as_ref()
            .ok_or("native-mask-route-not-selected")?;
        if route
            .carrier_presence_deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
            || super::super::super::current_time_millis()? >= route.expires_at_millis
        {
            return Err("native-mask-return-expired".into());
        }
        let credential = &route.credential;
        if !request.has_exact_basis()
            || request.credential_id != credential.credential_id.as_str()
            || request.body_id != credential.body_id
            || request.part_id != credential.part_id
            || request.host_id != credential.host_id
            || request.boot_id != credential.boot_id
        {
            return Err("native-mask-route-credential-mismatch".into());
        }
        let face = self.face_snapshot(request)?;
        (if let Some(interaction_line) = &route.interaction_line {
            route.seal.validate_available_show_with_interaction(
                &self.session,
                &face,
                self.host.advertisement(),
                &route.host_offer,
                &route.face_line,
                &route.return_line,
                interaction_line,
                show,
            )
        } else {
            route.seal.validate_available_show(
                &self.session,
                &face,
                self.host.advertisement(),
                &route.host_offer,
                &route.face_line,
                &route.return_line,
                show,
            )
        })
        .map_err(|error| format!("native-mask-show-refused:{error:?}"))
    }
}
