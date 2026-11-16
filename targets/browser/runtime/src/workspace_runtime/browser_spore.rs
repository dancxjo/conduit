//! Transactional browser Host preparation in the current Body admission slot.
use super::*;

pub(super) fn prepare(
    candidate: &BodyLifecycleSession,
    request: Request,
) -> Result<Vec<u8>, Refusal> {
    let Request::PrepareBrowserSpore {
        host_id,
        boot_id,
        secret,
        nonce,
        now_millis,
        expires_at_millis,
        image_content_digest,
        selection,
    } = request
    else {
        return Err("Browser preparation requires its exact request".into());
    };
    let secret_bytes = <[u8; 32]>::try_from(secret.as_slice()).map_err(|_| {
        Refusal::new(
            "Admission.WeakSecret",
            "Invitation secret must have exactly 32 bytes",
        )
    })?;
    let invitation_secret =
        SpawnInvitationSecret::from_csprng_bytes(secret_bytes).map_err(|error| {
            Refusal::new(
                &format!("Admission.{error:?}"),
                "Invitation entropy was refused",
            )
        })?;
    let mut next_admissions = ADMISSIONS.with(|admissions| {
        admissions
            .borrow()
            .clone()
            .ok_or("Workspace admission state is missing")
    })?;
    let claim = candidate
        .issue_invitation(
            &mut next_admissions,
            invitation_secret,
            nonce,
            now_millis,
            expires_at_millis,
            &host_id,
            &boot_id,
        )
        .map_err(debug)?;
    let prepared = crate::creche::prepare_workspace_browser(
        candidate.evidence(),
        &claim,
        &secret_bytes,
        &image_content_digest,
        selection,
        now_millis,
    )
    .map_err(|message| Refusal::new("Make.Prepare", &message))?;
    let response = encode(&prepared)?;
    ADMISSIONS.with(|admissions| *admissions.borrow_mut() = Some(next_admissions));
    Ok(response)
}
