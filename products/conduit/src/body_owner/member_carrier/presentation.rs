//! Explicit native Mask selection on an actual authenticated, present carrier.
//! Worker-supplied Ready Lines remain actual transport-owner evidence; membership
//! authentication alone never calls this entrance or constructs those offers.
use super::*;
use conduit_core::{LineOffer, LinkBindingId, PlanId};
use conduit_face::{MaskShow, OwnerFaceSnapshotRequest, RemoteOwnerMaskRouteSeal};
impl Owner {
    fn present_member_carrier(
        &self,
        session: &LinkBindingId,
    ) -> Result<(MemberPresentationBasis, Instant, u64), String> {
        self.inspect_current_member_carrier(session)?;
        let active = self
            .member_carrier
            .as_ref()
            .and_then(|c| c.active.as_ref())
            .ok_or("member carrier absent")?;
        let presence = active
            .presence
            .as_ref()
            .ok_or("member carrier presence not started")?;
        let lease = presence
            .table
            .leases
            .first()
            .ok_or("member carrier presence absent")?;
        let deadline = presence.clock + Duration::from_millis(lease.expires_at_millis);
        if Instant::now() >= deadline {
            return Err("member carrier presence expired".into());
        }
        Ok((
            active.member.presentation(self)?,
            deadline.min(active.deadline),
            active.expires_at_millis,
        ))
    }

    pub(crate) fn select_member_carrier_mask_route(
        &mut self,
        session: &LinkBindingId,
        face_line: &LineOffer,
        return_line: &LineOffer,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let (member, deadline, expires) = self.present_member_carrier(session)?;
        let expected = crate::durable_host::routed_join::native_lines::offer_pair(
            self.host.advertisement(),
            &member,
            session.as_str(),
        );
        if expected.0 != *face_line || expected.1 != *return_line {
            return Err("member carrier Lines differ from actual session profile".into());
        }
        let seal = self.seal_native_mask_route(&member, face_line, return_line, expires)?;
        self.bind_native_carrier_presence(&seal.route_plan_id, deadline)?;
        self.member_carrier
            .as_mut()
            .and_then(|c| c.active.as_mut())
            .ok_or("member carrier absent")?
            .route = Some(seal.route_plan_id.clone());
        Ok(seal)
    }

    pub(crate) fn select_member_carrier_mask_route_with_interaction(
        &mut self,
        session: &LinkBindingId,
        face_line: &LineOffer,
        return_line: &LineOffer,
        interaction_line: Option<&LineOffer>,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let (member, deadline, expires) = self.present_member_carrier(session)?;
        let expected = crate::durable_host::routed_join::native_lines::offer_graphical(
            self.host.advertisement(),
            &member,
            session.as_str(),
        )?;
        if expected.0 != *face_line
            || expected.1 != *return_line
            || expected.2.as_ref() != interaction_line
        {
            return Err("member carrier Lines differ from actual session profile".into());
        }
        let seal = self.seal_graphical_native_mask_route(
            &member,
            face_line,
            return_line,
            interaction_line,
            expires,
        )?;
        self.bind_native_carrier_presence(&seal.route_plan_id, deadline)?;
        self.member_carrier
            .as_mut()
            .and_then(|c| c.active.as_mut())
            .ok_or("member carrier absent")?
            .route = Some(seal.route_plan_id.clone());
        Ok(seal)
    }

    pub(crate) fn acknowledge_member_carrier_show(
        &mut self,
        session: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
    ) -> Result<(), String> {
        let (_, deadline, _) = self.present_member_carrier(session)?;
        let route = self
            .member_carrier
            .as_ref()
            .and_then(|c| c.active.as_ref())
            .and_then(|a| a.route.as_ref())
            .ok_or("member carrier Mask not selected")?
            .clone();
        self.bind_native_carrier_presence(&route, deadline)?;
        self.acknowledge_native_mask_show(request, show)
    }

    /// Current member transport scope and actual presence remain distinct from
    /// the exact acknowledged Show and the semantic owner's action checks.
    pub(crate) fn validate_member_carrier_interaction(
        &mut self,
        session: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        interaction: &conduit_face::FaceInteraction,
    ) -> Result<(), String> {
        let (member, deadline, _) = self.present_member_carrier(session)?;
        let c = &member.credential;
        if !request.has_exact_basis()
            || request.credential_id != c.credential_id.as_str()
            || request.body_id != c.body_id
            || request.part_id != c.part_id
            || request.host_id != c.host_id
            || request.boot_id != c.boot_id
        {
            return Err("member interaction credential differs from actual carrier".into());
        }
        let route = self
            .member_carrier
            .as_ref()
            .and_then(|c| c.active.as_ref())
            .and_then(|active| active.route.as_ref())
            .ok_or("member carrier Mask not selected")?
            .clone();
        self.bind_native_carrier_presence(&route, deadline)?;
        self.validate_native_mask_interaction(request, show, interaction)
    }

    pub(super) fn refresh_member_carrier_route_presence(&mut self) -> Result<(), String> {
        let Some(active) = self.member_carrier.as_ref().and_then(|c| c.active.as_ref()) else {
            return Ok(());
        };
        let Some(route) = active.route.clone() else {
            return Ok(());
        };
        let session = active
            .session
            .inspection(&self.session.evidence().membership)
            .session_id;
        let (_, deadline, _) = self.present_member_carrier(&session)?;
        self.bind_native_carrier_presence(&route, deadline)
    }

    pub(super) fn retire_member_carrier_route(&mut self, route: Option<PlanId>) {
        if let Some(route) = route {
            self.retire_native_route_id(&route);
        }
    }
}
