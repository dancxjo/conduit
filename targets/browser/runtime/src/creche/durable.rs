//! Validation of serialized Crèche session evidence before restoration.

use super::session::DurableBodySession;

pub(super) fn validate(snapshot: &DurableBodySession) -> Result<(), String> {
    let receipt = &snapshot.receipt;
    if snapshot.schema != "conduit.creche/durable-session@1"
        || receipt.schema != "conduit.creche/body-birth@2"
        || receipt.disposition != "born"
        || receipt.state != "LULLED"
        || receipt.source_interaction.schema != "conduit.tour/source-interaction@1"
        || receipt.source_interaction.disposition != "accepted"
        || receipt.source_interaction.semantic_id != "interaction/executable-tour-source"
        || receipt.source_interaction.value_kind != conduit_human::TEXT_INFO_ID
        || receipt.initial_review.schema != "conduit.creche/initial-workload-review@1"
        || receipt.initial_review.disposition != "realizable"
        || receipt.initial_review.selected_form_count != receipt.initial_forms.len()
        || receipt.initial_review.authority_acquired
        || receipt.initial_review.resources_acquired
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
        || receipt.birth_sequence != receipt.raw_body.birth_sequence
        || receipt.workload_revision != receipt.raw_body.workload_revision
        || receipt.membership_revision != receipt.raw_membership.revision.0
        || receipt.raw_body != snapshot.biography.body
        || receipt.raw_membership != snapshot.biography.membership
        || receipt.body_id != snapshot.biography.body_id.as_str()
        || receipt.friendly_name != snapshot.biography.friendly_name
        || receipt.initial_forms.len() != receipt.raw_body.workset.len()
        || receipt.initial_forms.iter().any(|form| {
            !receipt
                .raw_body
                .workset
                .contains(&conduit_body::ResidentForm::new(
                    conduit_core::SourceDocumentId::from(form.source_document_id.clone()),
                    conduit_core::CheckedFormId::from(form.checked_form_id.clone()),
                ))
        })
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
