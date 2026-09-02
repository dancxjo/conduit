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
        schema: "conduit.creche/body-birth@1".into(),
        disposition: "born".into(),
        source_document_id: body.source_document_id.as_str().into(),
        checked_form_id: body.checked_form_id.as_str().into(),
        seed_id: body.seed_id.as_str().into(),
        body_id: body.body_id.as_str().into(),
        friendly_name: friendly_name.into(),
        initial_program: initial_program.into(),
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
        receipt.initial_program.clone(),
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
            .biography
            .can_append(2)
            .map_err(|error| format!("admit biography records: {error:?}"))?;
        let admitted_change = session
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
        let attached_change = session
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
        session
            .biography
            .append_membership_events(
                session.receipt.raw_membership.clone(),
                &[
                    (admitted_change, sequence),
                    (attached_change, attach_sequence),
                ],
            )
            .map_err(|error| format!("record Body membership biography: {error:?}"))?;
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
    validate_durable(&snapshot)?;
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

fn validate_durable(snapshot: &DurableBodySession) -> Result<(), String> {
    let receipt = &snapshot.receipt;
    if snapshot.schema != "conduit.creche/durable-session@1"
        || receipt.schema != "conduit.creche/body-birth@1"
        || receipt.disposition != "born"
        || receipt.state != "LULLED"
        || receipt.initial_program != "morse-network@1"
        || receipt.source_interaction.schema != "conduit.book/source-interaction@1"
        || receipt.source_interaction.disposition != "accepted"
        || receipt.source_interaction.semantic_id != "interaction/executable-book-source"
        || receipt.source_interaction.value_kind != conduit_human::TEXT_INFO_ID
    {
        return Err("durable Crèche session metadata is invalid".into());
    }
    receipt
        .raw_body
        .validate()
        .map_err(|error| format!("validate restored Body: {error:?}"))?;
    receipt
        .raw_membership
        .validate()
        .map_err(|error| format!("validate restored membership: {error:?}"))?;
    snapshot
        .biography
        .validate()
        .map_err(|error| format!("validate restored biography: {error:?}"))?;
    if receipt.body_id != receipt.raw_body.body_id.as_str()
        || receipt.seed_id != receipt.raw_body.seed_id.as_str()
        || receipt.source_document_id != receipt.raw_body.source_document_id.as_str()
        || receipt.checked_form_id != receipt.raw_body.checked_form_id.as_str()
        || receipt.birth_sequence != receipt.raw_body.birth_sequence
        || receipt.membership_revision != receipt.raw_membership.revision.0
        || receipt.raw_body != snapshot.biography.body
        || receipt.raw_membership != snapshot.biography.membership
        || receipt.body_id != snapshot.biography.body_id.as_str()
        || receipt.friendly_name != snapshot.biography.friendly_name
        || receipt.initial_program != snapshot.biography.initial_program
    {
        return Err("durable Crèche session identities disagree".into());
    }
    let graduation_matches = match (&receipt.graduation, &snapshot.biography.graduation) {
        (None, None) => true,
        (Some(receipt), Some(biography)) => {
            receipt.schema == "conduit.creche/graduation@1"
                && receipt.body_id == biography.body_id.as_str()
                && receipt.sequence == biography.sequence
                && receipt.sign_id == biography.sign_id.as_str()
                && receipt.patchbay_plan_id.as_deref()
                    == biography
                        .patchbay_plan_id
                        .as_ref()
                        .map(|value| value.as_str())
                && receipt.patchbay_implementation_id.as_deref()
                    == biography
                        .patchbay_implementation_id
                        .as_ref()
                        .map(|value| value.as_str())
                && matches!(
                    (receipt.choice.as_str(), &biography.choice),
                    (
                        "host-patchbay",
                        conduit_body::BodyGraduationChoice::HostedPatchbay
                    ) | (
                        "external-reader",
                        conduit_body::BodyGraduationChoice::ExternalReader
                    )
                )
                && !receipt.creche_required
        }
        _ => false,
    };
    if !graduation_matches {
        return Err("durable Crèche graduation evidence disagrees".into());
    }
    Ok(())
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
