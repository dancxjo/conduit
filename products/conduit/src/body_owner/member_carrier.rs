//! One finite current-member carrier, distinct from attachment and effect grants.
//! The caller must supply actual locally authorized transport facts. Authentication
//! here does not initialize a Line, start presence or authorize a Mask/Host Call.
use super::super::MemberPresentationBasis;
use super::Owner;
use conduit_body::{
    CurrentMemberChallengeSlot, CurrentMemberReceivingSession, FederationChallenge,
    FederationInspection, FederationPeer, MembershipCredential, SpawnInvitationSecret,
};
use conduit_core::{HostAdvertisement, LineId, LineSecurity};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct MemberCarrierCustody {
    key: Option<SpawnInvitationSecret>,
    pending: Option<Pending>,
    active: Option<Active>,
}
struct Pending {
    slot: CurrentMemberChallengeSlot,
    challenge: FederationChallenge,
    signature: [u8; 64],
    member: MemberPresentationBasis,
    deadline: Instant,
    session_deadline: Instant,
}
struct Active {
    session: CurrentMemberReceivingSession,
    expires_at_millis: u64,
    deadline: Instant,
    receiver: FederationPeer,
    member: MemberPresentationBasis,
}
impl Owner {
    fn member_carrier_peer(&self) -> Result<FederationPeer, String> {
        let key = self
            .member_carrier
            .as_ref()
            .and_then(|c| c.key.as_ref())
            .ok_or("member carrier owner key absent")?;
        let ad = self.host.advertisement();
        Ok(FederationPeer {
            host_id: ad.host_id.clone(),
            boot_id: ad.boot_id.clone(),
            offer_generation: ad.offer_generation,
            verifying_key: key.verifying_key(),
        })
    }

    /// A single occupied active slot refuses replacement; no implicit supersession.
    /// Challenge issuance and completion make no durable membership changes.
    pub(crate) fn begin_current_member_carrier(
        &mut self,
        credential: &MembershipCredential,
        advertisement: &HostAdvertisement,
        line: LineId,
        security: LineSecurity,
        maximum_millis: u32,
        maximum_frames: u32,
    ) -> Result<(FederationChallenge, [u8; 64]), String> {
        if !(1..=3_600_000).contains(&maximum_millis)
            || !(1..=65_536).contains(&maximum_frames)
            || line.as_str().is_empty()
            || line.as_str().len() > 256
            || security == LineSecurity::PlaintextNetwork
        {
            return Err("member carrier requires finite authorized transport facts".into());
        }
        let member = MemberPresentationBasis::new(credential.clone(), advertisement.clone())?;
        self.require_current_native_member(&member)?;
        let manager = self
            .admissions
            .as_ref()
            .ok_or("member admission custody absent")?;
        let witness = manager
            .current_member_continuity(
                &self.session.evidence().membership,
                &credential.part_id,
                advertisement,
            )
            .map_err(|e| format!("current member: {e:?}"))?;
        let started = Instant::now();
        let now = crate::durable_host::current_time_millis()?;
        let active_current = self
            .member_carrier
            .as_ref()
            .and_then(|c| c.active.as_ref())
            .is_some_and(|active| {
                self.inspect_current_member_carrier(
                    &active
                        .session
                        .inspection(&self.session.evidence().membership)
                        .session_id,
                )
                .is_ok()
            });
        let custody = self
            .member_carrier
            .get_or_insert_with(|| Box::new(MemberCarrierCustody::default()));
        if custody
            .pending
            .as_ref()
            .is_some_and(|p| Instant::now() < p.deadline)
        {
            return Err("member carrier challenge slot occupied".into());
        }
        if active_current {
            return Err("member carrier session slot occupied".into());
        }
        custody.pending = None;
        custody.active = None;
        if custody.key.is_none() {
            let mut bytes = [0; 32];
            getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
            custody.key = Some(
                SpawnInvitationSecret::from_csprng_bytes(bytes)
                    .map_err(|e| format!("owner key: {e:?}"))?,
            );
            bytes.fill(0);
        }
        let receiver = self.member_carrier_peer()?;
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        if nonce == [0; 32] {
            return Err("member carrier nonce unavailable".into());
        }
        let challenge = FederationChallenge {
            session_id: format!("member-carrier/{}", crate::durable_host::digest(&nonce)).into(),
            line_id: line,
            line_security: security,
            initiator: witness.peer().clone(),
            responder: receiver.clone(),
            nonce,
            expires_at_millis: now
                .checked_add(u64::from(maximum_millis))
                .ok_or("carrier expiry overflow")?,
            maximum_frames,
        };
        let mut slot = CurrentMemberChallengeSlot::default();
        slot.issue(witness, &receiver, challenge.clone(), now)
            .map_err(|e| format!("carrier issuance: {e:?}"))?;
        let transcript: [u8; 32] = conduit_body::challenge_transcript(&challenge)
            .try_into()
            .map_err(|_| "carrier transcript size")?;
        let custody = self
            .member_carrier
            .as_mut()
            .ok_or("carrier custody absent")?;
        let signature = custody
            .key
            .as_ref()
            .ok_or("carrier key absent")?
            .sign(&transcript);
        custody.pending = Some(Pending {
            slot,
            challenge: challenge.clone(),
            signature,
            member,
            deadline: started + Duration::from_secs(10),
            session_deadline: started + Duration::from_millis(u64::from(maximum_millis)),
        });
        Ok((challenge, signature))
    }

    pub(crate) fn complete_current_member_carrier(
        &mut self,
        challenge: &FederationChallenge,
        signature: [u8; 64],
    ) -> Result<FederationInspection, String> {
        let receiver = self.member_carrier_peer()?;
        let custody = self
            .member_carrier
            .as_mut()
            .ok_or("carrier custody absent")?;
        if custody
            .pending
            .as_ref()
            .is_none_or(|p| p.challenge != *challenge)
        {
            return Err("member carrier differs from pending issuance".into());
        }
        // An exact matching attempt consumes issuance even if scope/signature fails.
        let mut pending = custody.pending.take().ok_or("carrier challenge absent")?;
        if Instant::now() >= pending.deadline || Instant::now() >= pending.session_deadline {
            return Err("member carrier proof deadline expired".into());
        }
        self.require_current_native_member(&pending.member)?;
        let now = crate::durable_host::current_time_millis()?;
        if now >= challenge.expires_at_millis {
            return Err("member carrier session expired".into());
        }
        let session = pending
            .slot
            .complete(
                &self.session.evidence().membership,
                &receiver,
                challenge,
                signature,
                pending.signature,
                now,
            )
            .map_err(|e| format!("carrier proof: {e:?}"))?;
        let inspection = session.inspection(&self.session.evidence().membership);
        self.member_carrier
            .as_mut()
            .ok_or("carrier custody absent")?
            .active = Some(Active {
            session,
            expires_at_millis: challenge.expires_at_millis,
            deadline: pending.session_deadline,
            receiver,
            member: pending.member,
        });
        Ok(inspection)
    }

    /// Current authentication facts only; never an effect or Line grant.
    pub(crate) fn inspect_current_member_carrier(
        &self,
        session_id: &conduit_core::LinkBindingId,
    ) -> Result<FederationInspection, String> {
        let receiver = self.member_carrier_peer()?;
        let active = self
            .member_carrier
            .as_ref()
            .and_then(|c| c.active.as_ref())
            .ok_or("carrier session absent")?;
        self.require_current_native_member(&active.member)?;
        if active.receiver != receiver
            || Instant::now() >= active.deadline
            || crate::durable_host::current_time_millis()? >= active.expires_at_millis
        {
            return Err("carrier session is no longer current".into());
        }
        let inspection = active
            .session
            .inspection(&self.session.evidence().membership);
        if inspection.session_id != *session_id
            || !inspection.membership_current
            || !inspection.credential_current
        {
            return Err("carrier session scope differs".into());
        }
        Ok(inspection)
    }

    /// Drop only this exact authenticated session, never its member attachment.
    pub(crate) fn retire_current_member_carrier(
        &mut self,
        session_id: &conduit_core::LinkBindingId,
    ) -> Result<(), String> {
        let custody = self
            .member_carrier
            .as_mut()
            .ok_or("carrier custody absent")?;
        let active = custody.active.as_mut().ok_or("carrier session absent")?;
        if active
            .session
            .inspection(&self.session.evidence().membership)
            .session_id
            != *session_id
        {
            return Err("carrier retirement differs from active session".into());
        }
        active.session.revoke_credential();
        custody.active = None;
        Ok(())
    }
}
