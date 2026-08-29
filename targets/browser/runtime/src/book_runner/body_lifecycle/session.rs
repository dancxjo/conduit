use super::protocol::BirthReceipt;
use crate::book_runner::interaction::{admit_source, SourceInteractionEvidence};
use conduit_body::{AuthenticatedHostObservation, Body, BodyMembership, MembershipProofId, PartId};
use conduit_core::{bind_sign, BootId, HostId, OfferGeneration};
use std::cell::RefCell;

thread_local! {
    static BODY: RefCell<Option<BirthReceipt>> = const { RefCell::new(None) };
}

pub(super) fn birth(
    host: &str,
    boot: &str,
    source: &str,
    birth_sequence: u64,
    admitted_interaction: SourceInteractionEvidence,
) -> Result<BirthReceipt, String> {
    if birth_sequence == 0 {
        return Err("BIRTH sequence must be nonzero".into());
    }
    if current().is_some() {
        return Err("this Tour session already has a Body; duplicate BIRTH was refused".into());
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

    let here_part = PartId::bind(&body.body_id, "tour/here", 1)
        .map_err(|error| format!("bind Here Part: {error:?}"))?;
    let proof = MembershipProofId::bind(&admitted_interaction.result_identity)
        .map_err(|error| format!("bind membership proof: {error:?}"))?;
    let admit_sequence = birth_sequence
        .checked_add(1)
        .ok_or_else(|| "BIRTH sequence leaves no membership Sign capacity".to_string())?;
    let attach_sequence = birth_sequence
        .checked_add(2)
        .ok_or_else(|| "BIRTH sequence leaves no membership Sign capacity".to_string())?;
    let admit_sign = bind_sign(&host_id, &boot_id, None, admit_sequence);
    let attach_sign = bind_sign(&host_id, &boot_id, None, attach_sequence);
    let mut membership = BodyMembership::new(body.body_id.clone())
        .map_err(|error| format!("create Body membership: {error:?}"))?;
    membership
        .admit(
            &body.body_id,
            membership.revision,
            here_part.clone(),
            proof.clone(),
            admit_sign.sign_id,
        )
        .map_err(|error| format!("admit Here Part: {error:?}"))?;
    membership
        .observe_present(
            &body.body_id,
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
        birth_sequence,
        birth_sign_id: birth_sign.sign_id.as_str().into(),
        state: "LULLED",
        here_part_id: here_part.as_str().into(),
        host_id: host_id.as_str().into(),
        boot_id: boot_id.as_str().into(),
        membership_revision: membership.revision.0,
        wake_id: None,
        plan_id: None,
        active_play_id: None,
        source_interaction: admitted_interaction,
        raw_body: body,
        raw_membership: membership,
    };
    BODY.with(|slot| *slot.borrow_mut() = Some(receipt.clone()));
    Ok(receipt)
}

pub(super) fn current() -> Option<BirthReceipt> {
    BODY.with(|slot| slot.borrow().clone())
}

#[cfg(test)]
pub(super) fn clear_for_test() {
    BODY.with(|slot| *slot.borrow_mut() = None);
}
