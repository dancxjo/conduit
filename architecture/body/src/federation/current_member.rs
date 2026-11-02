//! Current-member binding above the existing trusted-key federation session.
use super::*;
use crate::{BodyId, BodyMembership, CurrentMemberContinuity, PartId};

/// A receiving session whose remote key was resolved from owner-local admission
/// custody. The wrapped legacy entrance is inaccessible, so authorization always
/// rechecks the exact Body and Part instead of accepting any matching Host.
pub struct CurrentMemberReceivingSession {
    session: FederatedReceivingSession,
    body_id: BodyId,
    part_id: PartId,
}
impl FederatedReceivingSession {
    /// Bind received challenge facts to trusted current-member and local receiver
    /// facts before verifying signatures. `expected_responder` must come from the
    /// actual local Host/key owner, never from the received challenge itself.
    ///
    /// The caller still owns fresh one-use challenge issuance/consumption, finite
    /// session bounds, current transport/Line evidence and receiver key pinning.
    /// The unchanged federation transcript contains no Body/Part fields: that
    /// context must be bound by the caller's challenge custody. This entrance
    /// alone proves neither challenge freshness nor an authorized/encrypted Line.
    pub fn establish_for_current_member(
        member: CurrentMemberContinuity,
        expected_responder: &FederationPeer,
        challenge: FederationChallenge,
        initiator_signature: [u8; 64],
        responder_signature: [u8; 64],
        now_millis: u64,
    ) -> Result<CurrentMemberReceivingSession, FederationRefusal> {
        if &challenge.initiator != member.peer() {
            return Err(FederationRefusal::PeerNotCurrentMember);
        }
        if &challenge.responder != expected_responder {
            return Err(FederationRefusal::Redirected);
        }
        let body_id = member.body_id().clone();
        let part_id = member.part_id().clone();
        let session = Self::establish(
            challenge,
            initiator_signature,
            responder_signature,
            now_millis,
        )?;
        Ok(CurrentMemberReceivingSession {
            session,
            body_id,
            part_id,
        })
    }
}
impl CurrentMemberReceivingSession {
    fn current_parts<'a>(&self, membership: &'a BodyMembership) -> Option<&'a [PartMembership]> {
        if membership.body_id != self.body_id {
            return None;
        }
        let mut matching = membership
            .parts
            .iter()
            .filter(|part| part.part_id == self.part_id);
        let part = matching.next()?;
        if matching.next().is_some()
            || !current_member(
                core::slice::from_ref(part),
                &self.session.challenge.initiator,
            )
        {
            return None;
        }
        Some(core::slice::from_ref(part))
    }
    pub fn inspection(&self, membership: &BodyMembership) -> FederationInspection {
        self.session
            .inspection(self.current_parts(membership).unwrap_or(&[]))
    }
    pub fn revoke_credential(&mut self) {
        self.session.revoke_credential();
    }
    #[allow(clippy::too_many_arguments)] // Existing session authorization plus exact Body scope.
    pub fn authorize(
        &mut self,
        frame: &FederatedOperationFrame,
        signature: [u8; 64],
        now_millis: u64,
        membership: &BodyMembership,
        capabilities: &mut BaseCapabilityTable,
        handle: &BaseCapabilityHandle,
    ) -> Result<BaseOperationLease, FederationRefusal> {
        let parts = self
            .current_parts(membership)
            .ok_or(FederationRefusal::PeerNotCurrentMember)?;
        self.session
            .authorize(frame, signature, now_millis, parts, capabilities, handle)
    }
}
