//! Atomic adult Form-workload changes at the ordinary Patchbay boundary.
//!
//! This session owns no second Body truth. Each accepted transition delegates
//! to the Body lifecycle, appends its exact Sign to retained biography
//! evidence, and publishes the resulting bounded evidence document.

use conduit_body::{
    BodyBiographyError, BodyBiographyEvidence, BodyId, BodyLifecycleError, BodyState, ResidentForm,
};
use conduit_core::SignId;

use crate::{
    PatchbayBodyApplicationEntrance, PatchbayBodyAttachment, PatchbayBodyEntranceError,
    MAX_PATCHBAY_BODY_EVIDENCE_BYTES,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyWorkloadChangeKind {
    Admitted,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyWorkloadChange {
    pub body_id: BodyId,
    pub prior_workload_revision: u64,
    pub workload_revision: u64,
    pub form: ResidentForm,
    pub sign_id: SignId,
    pub biography_sequence: u64,
    pub kind: BodyWorkloadChangeKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PatchbayBodyWorkloadError {
    Entrance(PatchbayBodyEntranceError),
    StaleWorkloadRevision { current: u64, offered: u64 },
    BodyAwake,
    Lifecycle(BodyLifecycleError),
    Biography(BodyBiographyError),
    EvidenceEncoding,
    EvidenceTooLarge,
}

#[derive(Debug, Clone)]
pub struct PatchbayBodyWorkloadSession {
    entrance: PatchbayBodyApplicationEntrance,
    evidence: BodyBiographyEvidence,
    encoded: Vec<u8>,
}

impl PatchbayBodyWorkloadSession {
    pub fn open_serialized(
        encoded: &[u8],
        entrance: PatchbayBodyApplicationEntrance,
    ) -> Result<Self, PatchbayBodyWorkloadError> {
        let attachment = PatchbayBodyAttachment::open_serialized(encoded, entrance.clone())
            .map_err(PatchbayBodyWorkloadError::Entrance)?;
        Ok(Self {
            entrance,
            evidence: attachment.evidence().clone(),
            encoded: encoded.to_vec(),
        })
    }

    pub fn evidence(&self) -> &BodyBiographyEvidence {
        &self.evidence
    }

    pub fn encoded_evidence(&self) -> &[u8] {
        &self.encoded
    }

    pub fn entrance(&self) -> &PatchbayBodyApplicationEntrance {
        &self.entrance
    }

    /// Publish an exact lifecycle extension without dropping workload or
    /// membership evidence. A failed append or encoded-size check is atomic.
    pub fn retain_wake(
        &mut self,
        body: conduit_body::Body,
        wake: conduit_body::Wake,
        first_sequence: u64,
    ) -> Result<(), PatchbayBodyWorkloadError> {
        let mut next = self.evidence.clone();
        next.append_wake(body, wake, first_sequence)
            .map_err(PatchbayBodyWorkloadError::Biography)?;
        let encoded =
            serde_json::to_vec(&next).map_err(|_| PatchbayBodyWorkloadError::EvidenceEncoding)?;
        if encoded.len() > MAX_PATCHBAY_BODY_EVIDENCE_BYTES {
            return Err(PatchbayBodyWorkloadError::EvidenceTooLarge);
        }
        self.evidence = next;
        self.encoded = encoded;
        Ok(())
    }

    pub fn admit_form(
        &mut self,
        expected_workload_revision: u64,
        form: ResidentForm,
        sign_id: SignId,
        biography_sequence: u64,
    ) -> Result<BodyWorkloadChange, PatchbayBodyWorkloadError> {
        self.change(
            expected_workload_revision,
            form,
            sign_id,
            biography_sequence,
            BodyWorkloadChangeKind::Admitted,
        )
    }

    pub fn remove_form(
        &mut self,
        expected_workload_revision: u64,
        form: ResidentForm,
        sign_id: SignId,
        biography_sequence: u64,
    ) -> Result<BodyWorkloadChange, PatchbayBodyWorkloadError> {
        self.change(
            expected_workload_revision,
            form,
            sign_id,
            biography_sequence,
            BodyWorkloadChangeKind::Removed,
        )
    }

    fn change(
        &mut self,
        expected_workload_revision: u64,
        form: ResidentForm,
        sign_id: SignId,
        biography_sequence: u64,
        kind: BodyWorkloadChangeKind,
    ) -> Result<BodyWorkloadChange, PatchbayBodyWorkloadError> {
        let current = self.evidence.body.workload_revision;
        if expected_workload_revision != current {
            return Err(PatchbayBodyWorkloadError::StaleWorkloadRevision {
                current,
                offered: expected_workload_revision,
            });
        }
        if self.evidence.body.state != BodyState::Lulled {
            return Err(PatchbayBodyWorkloadError::BodyAwake);
        }

        let next_body = match kind {
            BodyWorkloadChangeKind::Admitted => {
                self.evidence.body.admit_form(form.clone(), sign_id.clone())
            }
            BodyWorkloadChangeKind::Removed => {
                self.evidence.body.remove_form(&form, sign_id.clone())
            }
        }
        .map_err(PatchbayBodyWorkloadError::Lifecycle)?;
        let mut next_evidence = self.evidence.clone();
        next_evidence
            .append_body_workload_events(next_body, &[(sign_id.clone(), biography_sequence)])
            .map_err(PatchbayBodyWorkloadError::Biography)?;
        let next_encoded = serde_json::to_vec(&next_evidence)
            .map_err(|_| PatchbayBodyWorkloadError::EvidenceEncoding)?;
        if next_encoded.len() > MAX_PATCHBAY_BODY_EVIDENCE_BYTES {
            return Err(PatchbayBodyWorkloadError::EvidenceTooLarge);
        }

        let change = BodyWorkloadChange {
            body_id: next_evidence.body_id.clone(),
            prior_workload_revision: current,
            workload_revision: next_evidence.body.workload_revision,
            form,
            sign_id,
            biography_sequence,
            kind,
        };
        self.evidence = next_evidence;
        self.encoded = next_encoded;
        Ok(change)
    }
}

#[cfg(test)]
mod tests;
