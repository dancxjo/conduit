//! One bounded owner-local issuance, distinct from transport and effect authority.
use super::*;
use crate::{BodyMembership, CurrentMemberContinuity, CurrentMemberReceivingSession};

#[derive(Default)]
pub struct CurrentMemberChallengeSlot {
    pending: Option<(CurrentMemberContinuity, FederationChallenge)>,
    consumed: bool,
}
impl CurrentMemberChallengeSlot {
    /// Retain one challenge built from actual caller-owned nonce/session/Line
    /// facts. Nonce unpredictability and globally fresh session issuance remain
    /// the caller's obligation. A consumed slot can never be rearmed.
    pub fn issue(
        &mut self,
        member: CurrentMemberContinuity,
        local_responder: &FederationPeer,
        challenge: FederationChallenge,
        now_millis: u64,
    ) -> Result<&FederationChallenge, FederationRefusal> {
        if self.consumed {
            return Err(FederationRefusal::ChallengeConsumed);
        }
        if self.pending.is_some() {
            return Err(FederationRefusal::ChallengePending);
        }
        validate_challenge(&challenge, now_millis)?;
        if &challenge.initiator != member.peer() || &challenge.responder != local_responder {
            return Err(FederationRefusal::ChallengeContextMismatch);
        }
        self.pending = Some((member, challenge));
        Ok(&self.pending.as_ref().expect("retained issuance").1)
    }

    /// Consume the exact matching attempt before checking context or signatures.
    /// Current local responder facts must come from the actual key owner, never
    /// the received challenge. No successful session is a Line/capability grant.
    #[allow(clippy::too_many_arguments)]
    pub fn complete(
        &mut self,
        membership: &BodyMembership,
        local_responder: &FederationPeer,
        received: &FederationChallenge,
        initiator_signature: [u8; 64],
        responder_signature: [u8; 64],
        now_millis: u64,
    ) -> Result<CurrentMemberReceivingSession, FederationRefusal> {
        if self.consumed {
            return Err(FederationRefusal::ChallengeConsumed);
        }
        let Some((_, challenge)) = &self.pending else {
            return Err(FederationRefusal::WrongSession);
        };
        if challenge != received {
            return Err(FederationRefusal::WrongSession);
        }
        let (member, challenge) = self.pending.take().expect("matching issuance");
        self.consumed = true;
        validate_challenge(&challenge, now_millis)?;
        if &challenge.responder != local_responder || membership.body_id != *member.body_id() {
            return Err(FederationRefusal::ChallengeContextMismatch);
        }
        let mut parts = membership
            .parts
            .iter()
            .filter(|p| &p.part_id == member.part_id());
        let part = parts
            .next()
            .ok_or(FederationRefusal::PeerNotCurrentMember)?;
        if parts.next().is_some() || !current_member(core::slice::from_ref(part), member.peer()) {
            return Err(FederationRefusal::PeerNotCurrentMember);
        }
        FederatedReceivingSession::establish_for_current_member(
            member,
            local_responder,
            challenge,
            initiator_signature,
            responder_signature,
            now_millis,
        )
    }
}
