//! Closed per-carrier identities and bounded presence history. This is memory
//! backing only: a reservation grants no slot, membership, Line, Show or effect.
use super::*;
use conduit_body::{HostPresenceEvent, HostPresenceLease, MAX_PRESENCE_EVENTS};
use core::mem::size_of;

pub(super) const SESSION_ID_BYTES: usize = "member-carrier/".len() + 64;
pub(super) const PRESENCE_SIGN_BYTES: usize = SESSION_ID_BYTES + "/presence/".len() + 20;

/// A private checked envelope. Callers cannot substitute a guessed budget.
pub(super) struct CarrierBacking {
    pub(super) bytes: usize,
}
impl CarrierBacking {
    /// Source strings stay borrowed and independently funded. Every descendant
    /// created by the issuance/completion/presence factories below is a bounded
    /// clone of these exact current records or one fixed generated identity.
    pub(super) fn checked(
        credential: &MembershipCredential,
        retained_offer: &HostAdvertisement,
        local: &HostAdvertisement,
        manager: &conduit_body::AdmissionManager,
        membership: &conduit_body::BodyMembership,
        line: &LineId,
    ) -> Option<Self> {
        let part = membership
            .parts
            .iter()
            .find(|part| part.part_id == credential.part_id)?;
        let observation = part.current.as_ref()?;
        let pin = retained_offer
            .host_id
            .0
            .capacity()
            .checked_add(retained_offer.boot_id.0.capacity())?;
        let member = credential.retained_backing_bytes()?.checked_add(pin)?;
        let initiator = observation
            .host_id
            .0
            .capacity()
            .checked_add(observation.boot_id.0.capacity())?;
        let responder = local
            .host_id
            .0
            .capacity()
            .checked_add(local.boot_id.0.capacity())?;
        let continuity = manager
            .body_id
            .retained_backing_bytes()
            .checked_add(credential.part_id.retained_backing_bytes())?
            .checked_add(initiator)?;
        let challenge = SESSION_ID_BYTES
            .checked_add(line.0.capacity())?
            .checked_add(initiator)?
            .checked_add(responder)?;
        // Pending owns its explicit challenge plus the slot's one-use copy and
        // continuity. Completion additionally clones Body/Part while consuming
        // continuity; receiver peer ownership is retained by Active.
        let mut bytes = size_of::<Pending>()
            .checked_add(size_of::<Active>())?
            .checked_add(member)?
            .checked_add(challenge)?
            .checked_add(challenge)?
            .checked_add(challenge)? // returned challenge overlaps retained issuance
            .checked_add(continuity)?
            .checked_add(manager.body_id.retained_backing_bytes())?
            .checked_add(credential.part_id.retained_backing_bytes())?
            .checked_add(responder)?;
        // Presence owns one exact lease and at most64 events. Start preallocates
        // those capacities; renewal removes before append and never reallocates.
        let lease_ids = credential
            .part_id
            .retained_backing_bytes()
            .checked_add(observation.host_id.0.capacity())?
            .checked_add(observation.boot_id.0.capacity())?
            .checked_add(observation.proof_id.retained_backing_bytes())?
            .checked_add(SESSION_ID_BYTES)?;
        let event = size_of::<HostPresenceEvent>()
            .checked_add(lease_ids)?
            .checked_add(PRESENCE_SIGN_BYTES)?;
        bytes = bytes
            .checked_add(size_of::<Presence>())?
            .checked_add(membership.body_id.retained_backing_bytes())?
            .checked_add(SESSION_ID_BYTES)?
            .checked_add(size_of::<HostPresenceLease>())?
            .checked_add(lease_ids)?
            .checked_add(MAX_PRESENCE_EVENTS.checked_mul(event)?)?
            // One incoming renewal event overlaps all retained events before
            // commit_event removes the oldest. The returned lease clone and
            // one inspection are finite synchronous factory results.
            .checked_add(event)?
            .checked_add(size_of::<HostPresenceLease>())?
            .checked_add(lease_ids)?
            .checked_add(size_of::<FederationInspection>())?
            .checked_add(SESSION_ID_BYTES)?
            .checked_add(line.0.capacity())?
            .checked_add(initiator)?
            // challenge_transcript is a streaming SHA256 and its sole Vec is
            // exactly the32-byte final digest, not an encoded challenge buffer.
            .checked_add(32)?;
        Some(Self { bytes })
    }
}

impl Owner {
    /// Construct the actual Pending payload inside the original Host's funded
    /// factory, then move that nonClone issuance together with the SAME lease.
    #[allow(clippy::too_many_arguments)] // Original carrier facts and unchanged clock samples.
    pub(super) fn begin_image_member_carrier(
        &mut self,
        credential: &MembershipCredential,
        incoming: &HostAdvertisement,
        line: LineId,
        security: LineSecurity,
        expires_at_millis: u64,
        maximum_frames: u32,
        started: Instant,
        now: u64,
    ) -> Result<(FederationChallenge, [u8; 64]), String> {
        self.check_native_viewer_challenge(credential)?;
        let key = self
            .member_carrier
            .as_ref()
            .and_then(|custody| custody.key.as_ref())
            .ok_or("original IMAGE carrier receiver must be prepared before admission")?;
        let manager = self
            .admissions
            .as_ref()
            .ok_or("member admission custody absent")?;
        let offer = self
            .current_host_offers
            .current_hosts(self.session.evidence())
            .find(|offer| *offer == incoming)
            .ok_or("member offer has no original current custody")?;
        let membership = &self.session.evidence().membership;
        let backing = CarrierBacking::checked(
            credential,
            offer,
            self.host.advertisement(),
            manager,
            membership,
            &line,
        )
        .ok_or("unsupported or overflowing carrier backing")?;
        let proof_capacity = membership
            .parts
            .iter()
            .find(|part| part.part_id == credential.part_id)
            .and_then(|part| part.current.as_ref())
            .ok_or("member observation absent")?
            .proof_id
            .retained_backing_bytes();
        let maximum_millis = expires_at_millis.saturating_sub(now);
        // SAFETY: closed identity/continuity/challenge copies and32B transcript
        // are named by CarrierBacking. Later Active mutations are inline only;
        // one-Part presence has fixed Vec capacities and checked proof backing.
        // The actual Pending is created here after admission, not metadata
        // standing in for unknown future data. All effects are synchronous.
        let preparation = self.host.root_preparation()?.clone();
        let actual = self.host.advertisement();
        let prepared = unsafe {
            preparation.prepare_value(backing.bytes, backing.bytes, || {
                let receiver = FederationPeer {
                    host_id: actual.host_id.clone(),
                    boot_id: actual.boot_id.clone(),
                    offer_generation: actual.offer_generation,
                    verifying_key: key.verifying_key(),
                };
                let witness = manager
                    .current_member_continuity(membership, &credential.part_id, offer)
                    .map_err(|e| format!("current member: {e:?}"))?;
                let member = RetainedMember {
                    credential: credential.clone(),
                    offer: super::super::member_offers::PinnedMemberOffer::from_current_offer(
                        offer,
                    )?,
                };
                let mut nonce = [0; 32];
                getrandom::fill(&mut nonce).map_err(|_| "member carrier nonce unavailable")?;
                if nonce == [0; 32] {
                    return Err("member carrier nonce unavailable".into());
                }
                let mut session = String::with_capacity(SESSION_ID_BYTES);
                use std::fmt::Write;
                session.push_str("member-carrier/");
                for byte in nonce {
                    write!(&mut session, "{byte:02x}")
                        .map_err(|_| "carrier identity formatting failed")?;
                }
                let challenge = FederationChallenge {
                    session_id: session.into(),
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
                let signature = key.sign(&transcript);
                let session_deadline = started + Duration::from_millis(maximum_millis);
                Ok(std::cell::RefCell::new(Some(Pending {
                    slot,
                    challenge,
                    signature,
                    member,
                    deadline: (started + Duration::from_secs(10)).min(session_deadline),
                    session_deadline,
                    presence_proof_capacity: proof_capacity,
                    memory: None,
                })))
            })
        }?;
        let memory = prepared.retain_memory();
        let mut pending = prepared
            .value()
            .borrow_mut()
            .take()
            .ok_or("prepared issuance absent")?;
        pending.memory = Some(memory);
        let response = (pending.challenge.clone(), pending.signature);
        // Only after the complete replacement is funded and constructed do we
        // switch the selected lane or retire an expired exact predecessor.
        self.select_native_viewer_credential(credential)?;
        let custody = self
            .member_carrier
            .as_mut()
            .ok_or("carrier custody absent")?;
        let mut old_active = custody.active.take();
        let old_route = old_active.as_mut().and_then(|active| active.route.take());
        custody.pending = None;
        self.retire_member_carrier_route(old_route);
        drop(old_active);
        self.member_carrier
            .as_mut()
            .ok_or("carrier custody absent")?
            .pending = Some(pending);
        Ok(response)
    }
}
