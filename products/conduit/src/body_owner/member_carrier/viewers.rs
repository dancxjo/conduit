//! Two original viewer lanes. Switching is an allocation-free custody move,
//! never supersession, a second membership or a newly issued effect grant.
use super::super::{face_wardrobe::OwnerFaceWardrobe, native_mask_route::NativeMaskRoute};
use super::*;

/// The current lane stays in the original Owner fields so ordinary native
/// Show/action checks keep their exact ABI. Only its independently held peer
/// is parked here. Both lanes retain their original memory leases in payloads.
#[derive(Default)]
pub(in crate::durable_host::owner::controller) struct ParkedViewer {
    pending: Option<Pending>,
    active: Option<Active>,
    route: Option<NativeMaskRoute>,
    wardrobe: Option<OwnerFaceWardrobe>,
}
impl ParkedViewer {
    fn credential(&self) -> Option<&MembershipCredential> {
        self.pending
            .as_ref()
            .map(|pending| &pending.member.credential)
            .or_else(|| self.active.as_ref().map(|active| &active.member.credential))
            .or_else(|| self.route.as_ref().map(NativeMaskRoute::credential))
    }
    fn matches_session(&self, id: &conduit_core::LinkBindingId) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.challenge.session_id == *id)
            || self
                .active
                .as_ref()
                .is_some_and(|active| active.session.session_id() == id)
    }
}
impl Owner {
    pub(super) fn check_native_viewer_challenge(
        &self,
        credential: &MembershipCredential,
    ) -> Result<(), String> {
        let current = self.current_viewer_credential();
        let parked = self.parked_viewer.credential();
        let (pending, active) = if current == Some(credential) {
            let custody = self
                .member_carrier
                .as_ref()
                .ok_or("carrier custody absent")?;
            (custody.pending.as_ref(), custody.active.as_ref())
        } else if parked == Some(credential) {
            (
                self.parked_viewer.pending.as_ref(),
                self.parked_viewer.active.as_ref(),
            )
        } else {
            if current.is_some() && parked.is_some() {
                return Err("two original native viewer lanes are occupied".into());
            }
            if [current, parked]
                .into_iter()
                .flatten()
                .any(|old| old.part_id == credential.part_id || old.host_id == credential.host_id)
            {
                return Err("another incarnation of this native viewer still owns its lane".into());
            }
            return Ok(());
        };
        let now = Instant::now();
        if pending.is_some_and(|pending| now < pending.deadline) {
            return Err("member carrier challenge slot occupied".into());
        }
        if let Some(active) = active {
            let presence_current = active.presence.as_ref().is_none_or(|presence| {
                let elapsed = presence.clock.elapsed().as_millis();
                presence
                    .table
                    .leases
                    .first()
                    .is_some_and(|lease| elapsed < u128::from(lease.expires_at_millis))
            });
            if presence_current
                && now < active.deadline
                && active.session.is_current(
                    &self.session.evidence().membership,
                    crate::durable_host::current_time_millis()?,
                )
                && active.member.current_offer(self).is_ok()
            {
                return Err("member carrier session slot occupied".into());
            }
        }
        Ok(())
    }

    fn current_viewer_credential(&self) -> Option<&MembershipCredential> {
        self.member_carrier
            .as_ref()
            .and_then(|custody| {
                custody
                    .pending
                    .as_ref()
                    .map(|pending| &pending.member.credential)
                    .or_else(|| {
                        custody
                            .active
                            .as_ref()
                            .map(|active| &active.member.credential)
                    })
            })
            .or_else(|| {
                self.pending_native_mask
                    .as_ref()
                    .map(NativeMaskRoute::credential)
            })
    }

    fn swap_native_viewers(&mut self) -> Result<(), String> {
        let custody = self
            .member_carrier
            .as_mut()
            .ok_or("member carrier receiver absent")?;
        core::mem::swap(&mut custody.pending, &mut self.parked_viewer.pending);
        core::mem::swap(&mut custody.active, &mut self.parked_viewer.active);
        core::mem::swap(&mut self.pending_native_mask, &mut self.parked_viewer.route);
        core::mem::swap(&mut self.face_wardrobe, &mut self.parked_viewer.wardrobe);
        Ok(())
    }

    pub(in crate::durable_host::owner::controller) fn select_existing_native_viewer_credential(
        &mut self,
        credential: &MembershipCredential,
    ) -> Result<(), String> {
        if self.current_viewer_credential() == Some(credential) {
            return Ok(());
        }
        if self.parked_viewer.credential() == Some(credential) {
            return self.swap_native_viewers();
        }
        Err("native viewer credential does not own an existing lane".into())
    }

    /// Exact full credential match, never a scalar hash, Part label or matching
    /// Host alone. An occupied third lane cannot evict either original owner.
    pub(in crate::durable_host::owner::controller) fn select_native_viewer_credential(
        &mut self,
        credential: &MembershipCredential,
    ) -> Result<(), String> {
        if self.current_viewer_credential() == Some(credential) {
            return Ok(());
        }
        if self.parked_viewer.credential() == Some(credential) {
            return self.swap_native_viewers();
        }
        if self.current_viewer_credential().is_none() {
            return Ok(());
        }
        if self.parked_viewer.credential().is_some() {
            return Err("two original native viewer lanes are occupied".into());
        }
        // An IMAGE-backed second lane has to reserve its complete carrier
        // envelope before any swap publishes that lane. No diagnostic Host
        // without an actual selected IMAGE can obtain concurrent custody.
        if !self.host.has_image_custody() {
            return Err("concurrent native viewer requires original IMAGE custody".into());
        }
        self.swap_native_viewers()
    }

    pub(in crate::durable_host::owner::controller) fn select_native_viewer_session(
        &mut self,
        id: &conduit_core::LinkBindingId,
    ) -> Result<(), String> {
        let matches = self.member_carrier.as_ref().is_some_and(|custody| {
            custody
                .pending
                .as_ref()
                .is_some_and(|pending| pending.challenge.session_id == *id)
                || custody
                    .active
                    .as_ref()
                    .is_some_and(|active| active.session.session_id() == id)
        });
        if matches {
            return Ok(());
        }
        if self.parked_viewer.matches_session(id) {
            return self.swap_native_viewers();
        }
        Err("member viewer session scope differs".into())
    }

    pub(super) fn original_viewer_active(
        &self,
        id: &conduit_core::LinkBindingId,
    ) -> Option<&Active> {
        self.member_carrier
            .as_ref()
            .and_then(|custody| custody.active.as_ref())
            .filter(|active| active.session.session_id() == id)
            .or_else(|| {
                self.parked_viewer
                    .active
                    .as_ref()
                    .filter(|active| active.session.session_id() == id)
            })
    }
}

impl Owner {
    pub(crate) fn take_native_response_backing(
        &mut self,
    ) -> Option<conduit_std_host::body_execution::ImageMemoryRetention> {
        self.pending_native_response_backing.take()
    }
    pub(crate) fn native_response_preparation(
        &self,
    ) -> Option<&conduit_std_host::body_execution::ImageRootPreparationOwner> {
        self.host.optional_root_preparation()
    }
    pub(crate) fn native_reply_seal_bytes(
        seal: &conduit_face::RemoteOwnerMaskRouteSeal,
    ) -> Option<usize> {
        core::mem::size_of::<conduit_face::RemoteOwnerMaskRouteSeal>()
            .checked_add(super::super::native_mask_route::backing::seal_payload_bytes(seal)?)
    }
}

impl Owner {
    pub(crate) fn capture_native_issuer_reply(&mut self) {
        self.pending_native_response_backing = self
            .member_carrier
            .as_ref()
            .and_then(|custody| custody.base_memory.clone());
    }
    pub(crate) fn capture_native_carrier_reply(&mut self, id: &conduit_core::LinkBindingId) {
        let current = self.member_carrier.as_ref().and_then(|custody| {
            custody
                .pending
                .as_ref()
                .filter(|pending| pending.challenge.session_id == *id)
                .and_then(|pending| pending.memory.clone())
                .or_else(|| {
                    custody
                        .active
                        .as_ref()
                        .filter(|active| active.session.session_id() == id)
                        .and_then(|active| active._memory.clone())
                })
        });
        self.pending_native_response_backing = current.or_else(|| {
            self.parked_viewer
                .pending
                .as_ref()
                .filter(|pending| pending.challenge.session_id == *id)
                .and_then(|pending| pending.memory.clone())
                .or_else(|| {
                    self.parked_viewer
                        .active
                        .as_ref()
                        .filter(|active| active.session.session_id() == id)
                        .and_then(|active| active._memory.clone())
                })
        });
    }
    pub(crate) fn capture_native_route_reply(&mut self) {
        self.pending_native_response_backing = self
            .pending_native_mask
            .as_ref()
            .and_then(super::super::native_mask_route::NativeMaskRoute::backing_memory);
    }
}

pub(in crate::durable_host::owner::controller) struct NativeCarrierView<'a> {
    pub(in crate::durable_host::owner::controller) credential: &'a MembershipCredential,
    pub(in crate::durable_host::owner::controller) offer: &'a HostAdvertisement,
    pub(in crate::durable_host::owner::controller) pin:
        &'a super::super::member_offers::PinnedMemberOffer,
    pub(in crate::durable_host::owner::controller) deadline: Instant,
    pub(in crate::durable_host::owner::controller) expires: u64,
    pub(in crate::durable_host::owner::controller) template:
        &'a conduit_std_host::body_execution::ImagePreparedValue<
            conduit_conduitos_mask_offer::CanonicalNativeGraphicalTemplate,
        >,
}
impl Owner {
    pub(in crate::durable_host::owner::controller) fn native_template(
        &self,
    ) -> Option<
        &conduit_std_host::body_execution::ImagePreparedValue<
            conduit_conduitos_mask_offer::CanonicalNativeGraphicalTemplate,
        >,
    > {
        self.member_carrier
            .as_ref()
            .and_then(|custody| custody.canonical.as_ref())
    }
    pub(in crate::durable_host::owner::controller) fn install_native_template(
        &mut self,
        template: conduit_std_host::body_execution::ImagePreparedValue<
            conduit_conduitos_mask_offer::CanonicalNativeGraphicalTemplate,
        >,
    ) -> Result<(), String> {
        let custody = self
            .member_carrier
            .as_mut()
            .ok_or("original carrier issuer absent")?;
        if custody.canonical.is_some() {
            return Err("original native DATA template already retained".into());
        }
        custody.canonical = Some(template);
        Ok(())
    }
    pub(in crate::durable_host::owner::controller) fn native_carrier_view(
        &self,
        id: &conduit_core::LinkBindingId,
    ) -> Result<NativeCarrierView<'_>, String> {
        let active = self
            .original_viewer_active(id)
            .ok_or("original member carrier absent")?;
        let presence = active
            .presence
            .as_ref()
            .ok_or("member carrier presence absent")?;
        let lease = presence
            .table
            .leases
            .first()
            .ok_or("member carrier presence absent")?;
        let deadline =
            (presence.clock + Duration::from_millis(lease.expires_at_millis)).min(active.deadline);
        if Instant::now() >= deadline {
            return Err("member carrier presence expired".into());
        }
        Ok(NativeCarrierView {
            credential: &active.member.credential,
            offer: active.member.current_offer(self)?,
            pin: &active.member.offer,
            deadline,
            expires: active.expires_at_millis,
            template: self
                .native_template()
                .ok_or("canonical native DATA template absent")?,
        })
    }
    pub(in crate::durable_host::owner::controller) fn bind_original_native_route(
        &mut self,
        session: &conduit_core::LinkBindingId,
        plan: &conduit_core::PlanId,
    ) -> Result<(), String> {
        let active = self
            .member_carrier
            .as_mut()
            .and_then(|custody| custody.active.as_mut())
            .filter(|active| active.session.session_id() == session)
            .ok_or("native viewer session changed")?;
        active.route = Some(plan.clone());
        Ok(())
    }
}
