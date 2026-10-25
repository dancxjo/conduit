//! Active Images phase. Canonical interactions request effects; only an explicit
//! owner grant starts file work, and only off-kernel final verification finishes.
use super::*;
use crate::{
    creche_images_face::CrecheImagesFace,
    creche_production::{CrecheProducedImages, CrecheSavedReceipt},
};
use conduit_body_make::PreparedBirthArtifactReceipt;
use conduit_face::{FaceActionAvailability, FaceText, MAX_FACE_TOTAL_BYTES};
mod worker;

/// Exact pending canonical Save. This is a request for a grant, never a path or
/// filesystem authority. The owner supplies an already admitted empty RW file.
#[derive(Clone)]
pub struct CrechePendingSave {
    receipt: PreparedBirthArtifactReceipt,
}
impl CrechePendingSave {
    pub fn receipt(&self) -> &PreparedBirthArtifactReceipt {
        &self.receipt
    }
    pub fn host_entry(&self) -> &str {
        &self.receipt.host_entry
    }
}
pub(super) enum Pending {
    Save(Box<CrechePendingSave>),
    Done,
}
pub(super) struct ImagesRuntime {
    pending: Option<Pending>,
    busy: bool,
    saved: Vec<CrecheSavedReceipt>,
    notice: String,
    pub(super) evidence: Option<Vec<u8>>,
}
pub(super) fn install(state: &mut Shared, products: &CrecheProducedImages) -> Result<(), String> {
    let prior = state.face.as_ref().ok_or("Crèche current Face absent")?;
    let revision = prior
        .revision
        .checked_add(1)
        .ok_or("Crèche Face revision exhausted")?;
    let projection = CrecheImagesFace::new(
        products,
        prior.basis.clone(),
        revision,
        MAX_FACE_TOTAL_BYTES,
    )
    .map_err(|e| format!("Crèche Images Face: {e:?}"))?;
    state.face = Some(Arc::new(projection.face().clone()));
    state.acknowledged = None;
    state.images = Some(ImagesRuntime {
        pending: None,
        busy: false,
        saved: Vec::new(),
        notice: String::new(),
        evidence: None,
    });
    Ok(())
}
fn refresh(state: &mut Shared) -> Result<(), String> {
    let images = state.images.as_ref().ok_or("Crèche Images absent")?;
    let products = state.products.as_ref().ok_or("Crèche products absent")?;
    let prior = state.face.as_ref().ok_or("Crèche current Face absent")?;
    let revision = prior
        .revision
        .checked_add(1)
        .ok_or("Crèche Face revision exhausted")?;
    let projection = CrecheImagesFace::new(
        products,
        prior.basis.clone(),
        revision,
        MAX_FACE_TOTAL_BYTES,
    )
    .map_err(|e| format!("Crèche Images Face: {e:?}"))?;
    let face = projection.face();
    let mut actions = face.actions.clone();
    if images.busy || images.pending.is_some() {
        for action in &mut actions {
            action.availability = FaceActionAvailability::Unavailable {
                reason_code: "creche-image-operation-pending".into(),
                explanation: "Waiting for the admitted image owner".into(),
            };
        }
    }
    let mut text = face.text.clone();
    if !images.notice.is_empty() {
        text.push(FaceText {
            subject: face.subjects[0].identity.clone(),
            text: images.notice.clone(),
        });
    }
    let face = Face::new_with_semantics(
        revision,
        face.basis.clone(),
        face.subjects.clone(),
        face.relationships.clone(),
        face.properties.clone(),
        text,
        actions,
        face.disclosures.clone(),
    )
    .map_err(|e| format!("Crèche Images status Face: {e:?}"))?;
    if serde_json::to_vec(&face)
        .map_err(|_| "Crèche Images encoding")?
        .len()
        > MAX_FACE_TOTAL_BYTES
    {
        return Err("Crèche Images status bound exhausted".into());
    }
    state.face = Some(Arc::new(face));
    state.acknowledged = None;
    Ok(())
}
pub(super) fn consume(state: &mut Shared, command: &Command) -> Result<(), String> {
    let images = state.images.as_ref().ok_or("Crèche Images absent")?;
    if images.busy || images.pending.is_some() {
        return Err("Crèche Images occupied".into());
    }
    let products = state.products.as_ref().ok_or("Crèche products absent")?;
    let face = state.face.as_ref().ok_or("Crèche current Face absent")?;
    // Validate the actual status Face, then resolve only identities projected
    // from this exact sealed batch; status text does not change action identity.
    command
        .interaction
        .validate_against(face, &command.show)
        .map_err(|_| "Crèche stale Images interaction")?;
    let projection = CrecheImagesFace::new(
        products,
        face.basis.clone(),
        face.revision,
        MAX_FACE_TOTAL_BYTES,
    )
    .map_err(|_| "Crèche Images projection")?;
    let pending = if command.interaction.action_id
        == projection
            .face()
            .actions
            .last()
            .ok_or("Images Done absent")?
            .identity
    {
        Pending::Done
    } else {
        let action = projection
            .face()
            .actions
            .iter()
            .position(|a| a.identity == command.interaction.action_id)
            .ok_or("Crèche unknown Images action")?;
        let receipt = products
            .artifacts()
            .get(action)
            .ok_or("Crèche unknown Images Host")?
            .receipt()
            .clone();
        Pending::Save(Box::new(CrechePendingSave { receipt }))
    };
    let images = state.images.as_mut().unwrap();
    images.pending = Some(pending);
    images.notice =
        "Waiting for the admitted image owner. No boot or join has been observed.".into();
    refresh(state)
}
impl CrecheEncounterController {
    pub fn pending_save(&self) -> Result<Option<CrechePendingSave>, String> {
        let state = self.channel.lock()?;
        Ok(
            match state.images.as_ref().and_then(|i| i.pending.as_ref()) {
                Some(Pending::Save(request)) if state.active => Some(request.as_ref().clone()),
                _ => None,
            },
        )
    }
    pub fn finish_images_pending(&self) -> Result<bool, String> {
        let state = self.channel.lock()?;
        Ok(state.active
            && matches!(
                state.images.as_ref().and_then(|i| i.pending.as_ref()),
                Some(Pending::Done)
            ))
    }
    pub fn saved_receipts(&self) -> Result<Vec<CrecheSavedReceipt>, String> {
        Ok(self
            .channel
            .lock()?
            .images
            .as_ref()
            .map(|i| i.saved.clone())
            .unwrap_or_default())
    }
    /// Recover the same sealed batch after retirement without claiming Prepared.
    /// Active operations transfer products only after canonical Done completion.
    pub fn take_retired_products(&self) -> Result<Option<CrecheProducedImages>, String> {
        let state = self.channel.lock()?;
        if !state.retirement_complete {
            return Err("Crèche worker and Mask retirement not complete".into());
        }
        drop(state);
        let worker = self
            .channel
            .worker
            .try_lock()
            .map_err(|_| "Crèche worker busy")?;
        if worker.handle.is_some() || worker.image_reserved {
            return Err("Crèche worker not drained".into());
        }
        drop(worker);
        Ok(self.channel.lock()?.products.take())
    }
}

/// A bounded envelope over the canonical prepared receipt evidence and actually
/// observed save receipts. It adds no carrier, boot or membership claim.
fn completion_evidence(
    products: &CrecheProducedImages,
    saved: &[CrecheSavedReceipt],
    maximum: u64,
) -> Result<Vec<u8>, String> {
    #[derive(serde::Serialize)]
    struct Evidence<'a> {
        schema: &'static str,
        prepared_evidence_sha256: String,
        prepared: serde_json::Value,
        saved: &'a [CrecheSavedReceipt],
        carrier_realized: bool,
        boot_observed: bool,
        join_observed: bool,
    }
    let prepared = products
        .evidence(maximum)
        .map_err(|_| "Crèche prepared evidence refused")?;
    let prepared_evidence_sha256 = format!("sha256:{:x}", Sha256::digest(&prepared));
    let prepared =
        serde_json::from_slice(&prepared).map_err(|_| "Crèche prepared evidence encoding")?;
    let evidence = serde_json::to_vec(&Evidence {
        schema: "conduit.creche/images-completion-evidence@1",
        prepared_evidence_sha256,
        prepared,
        saved,
        carrier_realized: false,
        boot_observed: false,
        join_observed: false,
    })
    .map_err(|_| "Crèche completion evidence encoding")?;
    if evidence.len() as u64 > maximum {
        return Err("Crèche completion evidence bound exhausted".into());
    }
    Ok(evidence)
}
