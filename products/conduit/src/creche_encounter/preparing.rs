//! Read-only canonical stage while the admitted make owner prepares images.
use super::*;
use conduit_face::{FaceActionAvailability, FaceText, MAX_FACE_TOTAL_BYTES};

pub(super) fn publish(state: &mut Shared) -> Result<(), String> {
    let prior = state
        .face
        .as_ref()
        .ok_or("Crèche current Make Face absent")?;
    let revision = prior
        .revision
        .checked_add(1)
        .ok_or("Crèche Face revision exhausted")?;
    let mut actions = prior.actions.clone();
    for action in &mut actions {
        action.availability = FaceActionAvailability::Unavailable {
            reason_code: "creche-image-preparation-pending".into(),
            explanation: "Canonical review accepted; awaiting admitted image preparation".into(),
        };
    }
    let mut text = prior.text.clone();
    text.push(FaceText {
        subject: prior.subjects.first().ok_or("Crèche Make subject absent")?.identity.clone(),
        text: "Review accepted. Waiting for the admitted image owner to prepare and verify the selected images. No prepared image, boot or membership result is claimed yet.".into(),
    });
    let face = Face::new_with_semantics(
        revision,
        prior.basis.clone(),
        prior.subjects.clone(),
        prior.relationships.clone(),
        prior.properties.clone(),
        text,
        actions,
        prior.disclosures.clone(),
    )
    .map_err(|e| format!("Crèche waiting Face: {e:?}"))?;
    if serde_json::to_vec(&face)
        .map_err(|_| "Crèche waiting Face encoding")?
        .len()
        > MAX_FACE_TOTAL_BYTES
    {
        return Err("Crèche waiting Face bound exhausted".into());
    }
    state.face = Some(Arc::new(face));
    state.acknowledged = None;
    Ok(())
}
