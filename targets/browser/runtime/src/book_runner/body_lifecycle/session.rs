use super::protocol::BirthReceipt;
use crate::book_runner::interaction::{admit_source, SourceInteractionEvidence};
use conduit_body::{
    AdmissionManager, AuthenticatedHostObservation, Body, BodyMembership, MembershipProofId, PartId,
};
use conduit_core::{bind_sign, BootId, HostId, OfferGeneration};
use std::cell::RefCell;

thread_local! {
    static BODY: RefCell<Option<BodySession>> = const { RefCell::new(None) };
}

pub(super) struct BodySession {
    pub(super) receipt: BirthReceipt,
    pub(super) admission: AdmissionManager,
    pub(super) pending_spore: Option<super::spore::PendingSpore>,
}

pub(super) fn birth(
    host: &str,
    boot: &str,
    friendly_name: &str,
    initial_program: &str,
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
    if initial_program != "morse-network@1" {
        return Err("the Crèche currently births only the reviewed Morse Network program".into());
    }
    let verified = admit_source(source.as_bytes(), admitted_interaction.sequence)?;
    if verified.proposal_identity != admitted_interaction.proposal_identity {
        return Err("source changed after typed interaction admission".into());
    }

    let (startup, _) = crate::installed_browser::catalogs()?;
    let syntax = conduit_form::parse_syntax_document(source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(format!("parse Body Seed Form: {}", diagnostic.message));
    }
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check Body Seed Form: {error:?}"))?;
    if checked.forms.len() != 1 {
        return Err("Body Seed source must contain exactly one checked Form".into());
    }
    let checked_form = &checked.forms[0];

    let host_id = HostId::from(host);
    let boot_id = BootId::from(boot);
    let birth_sign = bind_sign(&host_id, &boot_id, None, birth_sequence);
    let body = Body::born(
        checked.source_document_id.clone(),
        checked_form.checked_form_id.clone(),
        birth_sequence,
        birth_sign.sign_id.clone(),
    )
    .map_err(|error| format!("birth Body: {error:?}"))?;
    body.validate()
        .map_err(|error| format!("validate born Body: {error:?}"))?;

    let membership = BodyMembership::new(body.body_id.clone())
        .map_err(|error| format!("create Body membership: {error:?}"))?;
    membership
        .validate()
        .map_err(|error| format!("validate Body membership: {error:?}"))?;

    let receipt = BirthReceipt {
        schema: "conduit.book/body-birth@1",
        disposition: "born",
        source_document_id: body.source_document_id.as_str().into(),
        checked_form_id: body.checked_form_id.as_str().into(),
        seed_id: body.seed_id.as_str().into(),
        body_id: body.body_id.as_str().into(),
        friendly_name: friendly_name.into(),
        initial_program: initial_program.into(),
        birth_sequence,
        birth_sign_id: birth_sign.sign_id.as_str().into(),
        state: "LULLED",
        here_part_id: None,
        host_id: None,
        boot_id: None,
        membership_revision: membership.revision.0,
        wake_id: None,
        plan_id: None,
        active_play_id: None,
        source_interaction: admitted_interaction,
        raw_body: body,
        raw_membership: membership,
    };
    let admission = AdmissionManager::new(receipt.raw_body.body_id.clone())
        .map_err(|error| format!("create Body admission manager: {error:?}"))?;
    BODY.with(|slot| {
        *slot.borrow_mut() = Some(BodySession {
            receipt: receipt.clone(),
            admission,
            pending_spore: None,
        })
    });
    Ok(receipt)
}

pub(super) fn attach_here(host: &str, boot: &str, sequence: u64) -> Result<BirthReceipt, String> {
    with_session(|session| {
        if session.receipt.here_part_id.is_some() {
            return Ok(session.receipt.clone());
        }
        let host_id = HostId::from(host);
        let boot_id = BootId::from(boot);
        let here_part = PartId::bind(&session.receipt.raw_body.body_id, "creche/here", 1)
            .map_err(|error| format!("bind Here Part: {error:?}"))?;
        let proof = MembershipProofId::bind(&session.receipt.source_interaction.result_identity)
            .map_err(|error| format!("bind membership proof: {error:?}"))?;
        let admit_sign = bind_sign(&host_id, &boot_id, None, sequence);
        let attach_sequence = sequence
            .checked_add(1)
            .ok_or_else(|| "Host attachment sequence overflow".to_string())?;
        let attach_sign = bind_sign(&host_id, &boot_id, None, attach_sequence);
        session
            .receipt
            .raw_membership
            .admit(
                &session.receipt.raw_body.body_id,
                session.receipt.raw_membership.revision,
                here_part.clone(),
                proof.clone(),
                admit_sign.sign_id,
            )
            .map_err(|error| format!("admit Here Part: {error:?}"))?;
        session
            .receipt
            .raw_membership
            .observe_present(
                &session.receipt.raw_body.body_id,
                session.receipt.raw_membership.revision,
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
        session.receipt.here_part_id = Some(here_part.as_str().into());
        session.receipt.host_id = Some(host_id.as_str().into());
        session.receipt.boot_id = Some(boot_id.as_str().into());
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
