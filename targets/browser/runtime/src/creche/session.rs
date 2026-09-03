use super::protocol::BirthReceipt;
use crate::source_interaction::{admit_source, SourceInteractionEvidence};
use conduit_body::{
    AdmissionManager, AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyMembership,
    MembershipProofId, PartId,
};
use conduit_core::{bind_sign, BootId, HostId, OfferGeneration};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

thread_local! {
    static BODY: RefCell<Option<BodySession>> = const { RefCell::new(None) };
}

pub(super) struct BodySession {
    pub(super) receipt: BirthReceipt,
    pub(super) biography: BodyBiographyEvidence,
    pub(super) admission: AdmissionManager,
    pub(super) pending_spore: Option<super::spore::PendingSpore>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct DurableBodySession {
    pub(super) schema: String,
    pub(super) receipt: BirthReceipt,
    pub(super) biography: BodyBiographyEvidence,
}

pub(super) fn birth(
    host: &str,
    boot: &str,
    friendly_name: &str,
    initial_forms_json: &str,
    source: &str,
    birth_sequence: u64,
    admitted_interaction: SourceInteractionEvidence,
) -> Result<BirthReceipt, String> {
    if birth_sequence == 0 {
        return Err("BIRTH sequence must be nonzero".into());
    }
    if current().is_some() {
        return Err("this Crèche session already has a Body; duplicate BIRTH was refused".into());
    }
    let friendly_name = friendly_name.trim();
    if friendly_name.is_empty() || friendly_name.len() > 64 {
        return Err("friendly name must contain 1 through 64 UTF-8 bytes".into());
    }
    let verified = admit_source(source.as_bytes(), admitted_interaction.sequence)?;
    if verified.proposal_identity != admitted_interaction.proposal_identity {
        return Err("source changed after typed interaction admission".into());
    }

    let (workset, initial_forms) =
        super::initial_forms::checked_workset(source, initial_forms_json)?;

    let host_id = HostId::from(host);
    let boot_id = BootId::from(boot);
    let birth_sign = bind_sign(&host_id, &boot_id, None, birth_sequence);
    let body = Body::born_with_forms(workset, birth_sequence, birth_sign.sign_id.clone())
        .map_err(|error| format!("birth Body: {error:?}"))?;
    body.validate()
        .map_err(|error| format!("validate born Body: {error:?}"))?;

    let membership = BodyMembership::new(body.body_id.clone())
        .map_err(|error| format!("create Body membership: {error:?}"))?;
    membership
        .validate()
        .map_err(|error| format!("validate Body membership: {error:?}"))?;

    let receipt = BirthReceipt {
        schema: "conduit.creche/body-birth@2".into(),
        disposition: "born".into(),
        initial_forms,
        body_id: body.body_id.as_str().into(),
        friendly_name: friendly_name.into(),
        birth_sequence,
        birth_sign_id: birth_sign.sign_id.as_str().into(),
        state: "LULLED".into(),
        here_part_id: None,
        host_id: None,
        boot_id: None,
        membership_revision: membership.revision.0,
        wake_id: None,
        plan_id: None,
        active_play_id: None,
        source_interaction: admitted_interaction,
        graduation: None,
        raw_body: body,
        raw_membership: membership,
    };
    let admission = AdmissionManager::new(receipt.raw_body.body_id.clone())
        .map_err(|error| format!("create Body admission manager: {error:?}"))?;
    let biography = BodyBiographyEvidence::born(
        receipt.raw_body.clone(),
        receipt.raw_membership.clone(),
        receipt.friendly_name.clone(),
    )
    .map_err(|error| format!("create Body biography evidence: {error:?}"))?;
    BODY.with(|slot| {
        *slot.borrow_mut() = Some(BodySession {
            receipt: receipt.clone(),
            biography,
            admission,
            pending_spore: None,
        })
    });
    Ok(receipt)
}

pub(super) fn attach_here(host: &str, boot: &str, sequence: u64) -> Result<BirthReceipt, String> {
    with_session(|session| {
        let host_id = HostId::from(host);
        let boot_id = BootId::from(boot);
        if let Some(part) = session.receipt.here_part_id.clone() {
            let retained_host = session
                .receipt
                .host_id
                .as_deref()
                .ok_or_else(|| "restored Here membership has no Host identity".to_string())?;
            if retained_host != host_id.as_str() {
                return Err(
                    "current browser Host does not match the retained Here membership".into(),
                );
            }
            let retained = session
                .receipt
                .raw_membership
                .parts
                .iter()
                .find(|candidate| candidate.part_id.as_str() == part)
                .ok_or_else(|| "retained Here Part is absent from Body membership".to_string())?;
            let here_part = retained.part_id.clone();
            let current = retained.current.clone();
            if current
                .as_ref()
                .is_some_and(|observation| observation.boot_id == boot_id)
            {
                return Ok(session.receipt.clone());
            }
            return attach_admitted_here(session, here_part, host_id, boot_id, current, sequence);
        }
        let here_part = PartId::bind(&session.receipt.raw_body.body_id, "creche/here", 1)
            .map_err(|error| format!("bind Here Part: {error:?}"))?;
        let proof = MembershipProofId::bind(&session.receipt.source_interaction.result_identity)
            .map_err(|error| format!("bind membership proof: {error:?}"))?;
        let admit_sign = bind_sign(&host_id, &boot_id, None, sequence);
        let attach_sequence = sequence
            .checked_add(1)
            .ok_or_else(|| "Host attachment sequence overflow".to_string())?;
        let attach_sign = bind_sign(&host_id, &boot_id, None, attach_sequence);
        let mut membership = session.receipt.raw_membership.clone();
        let mut biography = session.biography.clone();
        biography
            .can_append(2)
            .map_err(|error| format!("admit biography records: {error:?}"))?;
        let admitted_change = membership
            .admit(
                &session.receipt.raw_body.body_id,
                membership.revision,
                here_part.clone(),
                proof.clone(),
                admit_sign.sign_id,
            )
            .map_err(|error| format!("admit Here Part: {error:?}"))?;
        let attached_change = membership
            .observe_present(
                &session.receipt.raw_body.body_id,
                membership.revision,
                &here_part,
                AuthenticatedHostObservation {
                    host_id: host_id.clone(),
                    boot_id: boot_id.clone(),
                    offer_generation: OfferGeneration(1),
                    proof_id: proof,
                    sequence: 1,
                },
                attach_sign.sign_id,
            )
            .map_err(|error| format!("observe current browser Host: {error:?}"))?;
        biography
            .append_membership_events(
                membership.clone(),
                &[
                    (admitted_change, sequence),
                    (attached_change, attach_sequence),
                ],
            )
            .map_err(|error| format!("record Body membership biography: {error:?}"))?;
        session.receipt.raw_membership = membership;
        session.biography = biography;
        session.receipt.here_part_id = Some(here_part.as_str().into());
        session.receipt.host_id = Some(host_id.as_str().into());
        session.receipt.boot_id = Some(boot_id.as_str().into());
        session.receipt.membership_revision = session.receipt.raw_membership.revision.0;
        Ok(session.receipt.clone())
    })
}

fn attach_admitted_here(
    session: &mut BodySession,
    here_part: PartId,
    host_id: HostId,
    boot_id: BootId,
    prior: Option<AuthenticatedHostObservation>,
    sequence: u64,
) -> Result<BirthReceipt, String> {
    let proof = MembershipProofId::bind(&session.receipt.source_interaction.result_identity)
        .map_err(|error| format!("bind membership proof: {error:?}"))?;
    let event_count = if prior.is_some() { 2 } else { 1 };
    let mut membership = session.receipt.raw_membership.clone();
    let mut biography = session.biography.clone();
    biography
        .can_append(event_count)
        .map_err(|error| format!("admit biography records: {error:?}"))?;
    let mut events = Vec::with_capacity(event_count);
    let mut attach_sequence = sequence;
    let observation_sequence = prior
        .as_ref()
        .map_or(1, |observation| observation.sequence + 1);
    if let Some(prior) = prior {
        if prior.host_id != host_id {
            return Err("current browser Host does not match the retained Here membership".into());
        }
        let detach_sign = bind_sign(&host_id, &prior.boot_id, None, sequence);
        let change = membership
            .observe_offline(
                &session.receipt.raw_body.body_id,
                membership.revision,
                &here_part,
                &prior.boot_id,
                detach_sign.sign_id,
            )
            .map_err(|error| format!("detach historical browser Boot: {error:?}"))?;
        events.push((change, sequence));
        attach_sequence = sequence
            .checked_add(1)
            .ok_or_else(|| "Host reattachment sequence overflow".to_string())?;
    }
    let attach_sign = bind_sign(&host_id, &boot_id, None, attach_sequence);
    let change = membership
        .observe_present(
            &session.receipt.raw_body.body_id,
            membership.revision,
            &here_part,
            AuthenticatedHostObservation {
                host_id: host_id.clone(),
                boot_id: boot_id.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: observation_sequence,
            },
            attach_sign.sign_id,
        )
        .map_err(|error| format!("observe current browser Host: {error:?}"))?;
    events.push((change, attach_sequence));
    biography
        .append_membership_events(membership.clone(), &events)
        .map_err(|error| format!("record Body membership biography: {error:?}"))?;
    session.receipt.raw_membership = membership;
    session.biography = biography;
    session.receipt.host_id = Some(host_id.as_str().into());
    session.receipt.boot_id = Some(boot_id.as_str().into());
    session.receipt.membership_revision = session.receipt.raw_membership.revision.0;
    Ok(session.receipt.clone())
}

pub(super) fn leave_here(host: &str, boot: &str, sequence: u64) -> Result<BirthReceipt, String> {
    with_session(|session| {
        let host_id = HostId::from(host);
        let boot_id = BootId::from(boot);
        let part = session
            .receipt
            .here_part_id
            .as_deref()
            .ok_or_else(|| "this Body has no admitted browser Part".to_string())?;
        if session.receipt.host_id.as_deref() != Some(host_id.as_str())
            || session.receipt.boot_id.as_deref() != Some(boot_id.as_str())
        {
            return Err("only the exact current browser Host and Boot may leave".into());
        }
        let here_part = session
            .receipt
            .raw_membership
            .parts
            .iter()
            .find(|candidate| candidate.part_id.as_str() == part)
            .ok_or_else(|| "retained Here Part is absent from Body membership".to_string())?
            .part_id
            .clone();
        let sign = bind_sign(&host_id, &boot_id, None, sequence);
        let mut membership = session.receipt.raw_membership.clone();
        let mut biography = session.biography.clone();
        biography
            .can_append(1)
            .map_err(|error| format!("admit biography record: {error:?}"))?;
        let change = membership
            .observe_offline(
                &session.receipt.raw_body.body_id,
                membership.revision,
                &here_part,
                &boot_id,
                sign.sign_id,
            )
            .map_err(|error| format!("detach current browser Boot: {error:?}"))?;
        biography
            .append_membership_events(membership.clone(), &[(change, sequence)])
            .map_err(|error| format!("record Body membership biography: {error:?}"))?;
        session.receipt.raw_membership = membership;
        session.biography = biography;
        session.receipt.boot_id = None;
        session.receipt.membership_revision = session.receipt.raw_membership.revision.0;
        Ok(session.receipt.clone())
    })
}

pub(super) fn revoke_here(host: &str, boot: &str, sequence: u64) -> Result<BirthReceipt, String> {
    with_session(|session| {
        let host_id = HostId::from(host);
        let part = session
            .receipt
            .here_part_id
            .as_deref()
            .ok_or_else(|| "this Body has no admitted browser Part".to_string())?;
        if session.receipt.host_id.as_deref() != Some(host_id.as_str()) {
            return Err("only the admitted browser Host may revoke its Part".into());
        }
        let boot_id = BootId::from(boot);
        if session
            .receipt
            .boot_id
            .as_deref()
            .is_some_and(|current| current != boot_id.as_str())
        {
            return Err("a stale browser Boot may not revoke the current Part".into());
        }
        let here_part = session
            .receipt
            .raw_membership
            .parts
            .iter()
            .find(|candidate| candidate.part_id.as_str() == part)
            .ok_or_else(|| "retained Here Part is absent from Body membership".to_string())?
            .part_id
            .clone();
        let sign = bind_sign(&host_id, &boot_id, None, sequence);
        let mut membership = session.receipt.raw_membership.clone();
        let mut biography = session.biography.clone();
        biography
            .can_append(1)
            .map_err(|error| format!("admit biography record: {error:?}"))?;
        let change = membership
            .revoke(
                &session.receipt.raw_body.body_id,
                membership.revision,
                &here_part,
                sign.sign_id,
            )
            .map_err(|error| format!("revoke browser Part: {error:?}"))?;
        biography
            .append_membership_events(membership.clone(), &[(change, sequence)])
            .map_err(|error| format!("record Body membership biography: {error:?}"))?;
        session.receipt.raw_membership = membership;
        session.biography = biography;
        session.receipt.boot_id = None;
        session.receipt.membership_revision = session.receipt.raw_membership.revision.0;
        Ok(session.receipt.clone())
    })
}

pub(super) fn current() -> Option<BirthReceipt> {
    BODY.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.receipt.clone())
    })
}

pub(super) fn biography() -> Option<BodyBiographyEvidence> {
    BODY.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.biography.clone())
    })
}

pub(super) fn durable_snapshot() -> Option<DurableBodySession> {
    BODY.with(|slot| {
        slot.borrow().as_ref().map(|session| DurableBodySession {
            schema: "conduit.creche/durable-session@1".into(),
            receipt: session.receipt.clone(),
            biography: session.biography.clone(),
        })
    })
}

pub(super) fn restore_durable(snapshot: DurableBodySession) -> Result<BirthReceipt, String> {
    if current().is_some() {
        return Err("this Crèche session already has a Body; durable restore was refused".into());
    }
    super::durable::validate(&snapshot)?;
    let admission = AdmissionManager::new(snapshot.receipt.raw_body.body_id.clone())
        .map_err(|error| format!("restore Body admission manager: {error:?}"))?;
    let receipt = snapshot.receipt;
    BODY.with(|slot| {
        *slot.borrow_mut() = Some(BodySession {
            receipt: receipt.clone(),
            biography: snapshot.biography,
            admission,
            pending_spore: None,
        });
    });
    Ok(receipt)
}

pub(super) fn forget_local() {
    BODY.with(|slot| slot.borrow_mut().take());
}

pub(super) fn with_session<T>(
    action: impl FnOnce(&mut BodySession) -> Result<T, String>,
) -> Result<T, String> {
    BODY.with(|slot| {
        let mut slot = slot.borrow_mut();
        let session = slot
            .as_mut()
            .ok_or_else(|| "BIRTH is required before preparing a physical Host".to_string())?;
        action(session)
    })
}

#[cfg(test)]
pub(super) fn clear_for_test() {
    BODY.with(|slot| *slot.borrow_mut() = None);
}
