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
#[path = "member_carrier/continuation.rs"]
mod continuation;
#[allow(dead_code)]
#[path = "member_carrier/preparation.rs"]
mod preparation;
#[path = "member_carrier/presentation.rs"]
mod presentation;

#[derive(Default)]
pub(super) struct MemberCarrierCustody {
    key: Option<SpawnInvitationSecret>,
    pending: Option<Pending>,
    active: Option<Active>,
    commands: Vec<preparation::PreparedRemoteBodyCommand>,
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
    presence: Option<Presence>,
    route: Option<conduit_core::PlanId>,
}
struct Presence {
    clock: Instant,
    table: conduit_body::HostPresenceTable,
}
const PRESENCE_LEASE_MILLIS: u64 = 2000;
impl Owner {
    pub(crate) fn member_carrier_peer(&self) -> Result<FederationPeer, String> {
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

    pub(crate) fn prepare_current_member_carrier_receiver(
        &mut self,
    ) -> Result<FederationPeer, String> {
        let custody = self
            .member_carrier
            .get_or_insert_with(|| Box::new(MemberCarrierCustody::default()));
        if custody.key.is_none() {
            let mut bytes = [0; 32];
            getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
            custody.key = Some(
                SpawnInvitationSecret::from_csprng_bytes(bytes)
                    .map_err(|e| format!("owner key: {e:?}"))?,
            );
            bytes.fill(0);
        }
        self.member_carrier_peer()
    }

    /// A single occupied active slot refuses replacement; no implicit supersession.
    /// Challenge issuance and completion make no durable membership changes.
    pub(crate) fn begin_current_member_carrier(
        &mut self,
        credential: &MembershipCredential,
        advertisement: &HostAdvertisement,
        line: LineId,
        security: LineSecurity,
        expires_at_millis: u64,
        maximum_frames: u32,
    ) -> Result<(FederationChallenge, [u8; 64]), String> {
        let started = Instant::now();
        let now = crate::durable_host::current_time_millis()?;
        let maximum_millis = expires_at_millis.saturating_sub(now);
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
        let active_current = self
            .member_carrier
            .as_ref()
            .and_then(|c| c.active.as_ref())
            .is_some_and(|active| {
                let presence_current = active.presence.as_ref().is_none_or(|presence| {
                    let now = presence.clock.elapsed().as_millis();
                    presence
                        .table
                        .leases
                        .first()
                        .is_some_and(|lease| now < u128::from(lease.expires_at_millis))
                });
                presence_current
                    && self
                        .inspect_current_member_carrier(
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
        let retired_route = custody
            .active
            .take()
            .and_then(|mut active| active.route.take());
        self.retire_member_carrier_route(retired_route);
        let receiver = self.prepare_current_member_carrier_receiver()?;
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
            expires_at_millis,
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
        let session_deadline = started + Duration::from_millis(maximum_millis);
        custody.pending = Some(Pending {
            slot,
            challenge: challenge.clone(),
            signature,
            member,
            deadline: (started + Duration::from_secs(10)).min(session_deadline),
            session_deadline,
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
            presence: None,
            route: None,
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

    /// The protected carrier worker may renew reachability only for its exact
    /// authenticated incarnation. This grants no Line, Mask or effect authority.
    /// A missed short lease requires fresh carrier authentication; a late packet
    /// cannot resurrect it merely because the overall session has time remaining.
    pub(crate) fn renew_current_member_presence(
        &mut self,
        session_id: &conduit_core::LinkBindingId,
        sequence: u64,
    ) -> Result<conduit_body::HostPresenceLease, String> {
        let inspection = self.inspect_current_member_carrier(session_id)?;
        if sequence == 0 || sequence > u64::from(inspection.maximum_frames) {
            return Err("member presence sequence exceeds carrier bounds".into());
        }
        let active = self
            .member_carrier
            .as_mut()
            .and_then(|custody| custody.active.as_mut())
            .ok_or("carrier session absent")?;
        let remaining = active
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        let lease_millis = PRESENCE_LEASE_MILLIS.min(remaining.min(u128::from(u64::MAX)) as u64);
        if lease_millis == 0 {
            return Err("member presence carrier expired".into());
        }
        let membership = &self.session.evidence().membership;
        let part = &active.member.credential.part_id;
        let sign =
            conduit_core::SignId::from(format!("{}/presence/{sequence}", session_id.as_str()));
        if let Some(presence) = active.presence.as_mut() {
            let now = presence
                .clock
                .elapsed()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64;
            if presence
                .table
                .leases
                .first()
                .is_none_or(|lease| now >= lease.expires_at_millis)
            {
                return Err("member presence lease expired".into());
            }
            presence
                .table
                .renew(
                    membership,
                    part,
                    session_id,
                    sequence,
                    now,
                    lease_millis,
                    sign,
                )
                .map_err(|error| format!("member presence renewal: {error:?}"))?;
        } else {
            if sequence != 1 {
                return Err("member presence must start at sequence one".into());
            }
            let clock = conduit_body::HostPresenceClock::new(
                session_id.as_str().into(),
                conduit_body::HostPresenceClockScale::Milliseconds,
                1,
                1,
            )
            .map_err(|error| format!("member presence clock: {error:?}"))?;
            let mut table = conduit_body::HostPresenceTable::new(
                membership.body_id.clone(),
                clock,
                PRESENCE_LEASE_MILLIS,
            )
            .map_err(|error| format!("member presence table: {error:?}"))?;
            let started = Instant::now();
            table
                .start(
                    membership,
                    part,
                    session_id.clone(),
                    sequence,
                    0,
                    lease_millis,
                    sign,
                )
                .map_err(|error| format!("member presence start: {error:?}"))?;
            active.presence = Some(Presence {
                clock: started,
                table,
            });
        }
        let lease = active
            .presence
            .as_ref()
            .and_then(|presence| presence.table.leases.first())
            .cloned()
            .ok_or("member presence absent")?;
        self.refresh_member_carrier_route_presence()?;
        Ok(lease)
    }

    /// Drop only this exact pending or authenticated session, never its member attachment.
    pub(crate) fn retire_current_member_carrier(
        &mut self,
        session_id: &conduit_core::LinkBindingId,
    ) -> Result<(), String> {
        let custody = self
            .member_carrier
            .as_mut()
            .ok_or("carrier custody absent")?;
        if custody
            .pending
            .as_ref()
            .is_some_and(|pending| pending.challenge.session_id == *session_id)
        {
            custody.pending = None;
            return Ok(());
        }
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
        let route = active.route.take();
        custody.active = None;
        self.retire_member_carrier_route(route);
        Ok(())
    }
}

impl Owner {
    /// Attests only an already committed member; this creates no membership or grant.
    pub(crate) fn attest_current_member_receipt(
        &mut self,
        receipt: &super::super::MembershipReceiptDocument,
        expected_owner: &FederationPeer,
    ) -> Result<conduit_body::SignedPortableAdmissionReceipt, String> {
        let member = receipt.presentation()?;
        self.require_current_native_member(&member)?;
        self.require_observed_member_offer(&member.host_advertisement)?;
        if let super::super::MembershipReceiptDocument::ImageReturn { receipt, .. } = receipt {
            let accepted: Vec<_> = self
                .current_host_offers
                .current_grants(self.session.evidence())
                .filter(|grant| {
                    grant.host_id == member.host_advertisement.host_id
                        && grant.boot_id == member.host_advertisement.boot_id
                })
                .collect();
            if accepted
                != receipt
                    .projection
                    .authority_grants
                    .iter()
                    .collect::<Vec<_>>()
            {
                return Err("image receipt grants differ from fresh committed return".into());
            }
        }
        let manager = self
            .admissions
            .as_ref()
            .ok_or("member admission custody absent")?;
        let witness = manager
            .current_member_continuity(
                &self.session.evidence().membership,
                &member.credential.part_id,
                &member.host_advertisement,
            )
            .map_err(|e| format!("current member: {e:?}"))?;
        let member_key = witness.peer().verifying_key;
        let owner = self.member_carrier_peer()?;
        if &owner != expected_owner {
            return Err("receipt owner differs from independently pinned owner".into());
        }
        let key = self
            .member_carrier
            .as_ref()
            .and_then(|c| c.key.as_ref())
            .ok_or("member carrier owner key absent")?;
        receipt.attest(owner, member_key, |t| key.sign(t))
    }
}
