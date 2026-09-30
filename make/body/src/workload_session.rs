//! Atomic, bounded changes to a Body's resident Form workset.

use conduit_body::{
    BodyBiographyError, BodyBiographyEvidence, BodyId, BodyLifecycleError, BodyState, ResidentForm,
};
use conduit_core::SignId;

pub const MAX_BODY_EVIDENCE_BYTES: usize = 32 * 1_024;

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
pub enum BodyWorkloadError {
    EmptyEvidence,
    EvidenceTooLarge,
    MalformedEvidence,
    InvalidEvidence,
    StaleWorkloadRevision { current: u64, offered: u64 },
    BodyAwake,
    Lifecycle(BodyLifecycleError),
    Biography(BodyBiographyError),
    EvidenceEncoding,
}

#[derive(Debug, Clone)]
pub struct BodyWorkloadSession {
    evidence: BodyBiographyEvidence,
    encoded: Vec<u8>,
}

impl BodyWorkloadSession {
    pub fn open_serialized(encoded: &[u8]) -> Result<Self, BodyWorkloadError> {
        if encoded.is_empty() {
            return Err(BodyWorkloadError::EmptyEvidence);
        }
        if encoded.len() > MAX_BODY_EVIDENCE_BYTES {
            return Err(BodyWorkloadError::EvidenceTooLarge);
        }
        let evidence: BodyBiographyEvidence =
            serde_json::from_slice(encoded).map_err(|_| BodyWorkloadError::MalformedEvidence)?;
        evidence
            .validate()
            .map_err(|_| BodyWorkloadError::InvalidEvidence)?;
        Ok(Self {
            evidence,
            encoded: encoded.to_vec(),
        })
    }

    pub fn evidence(&self) -> &BodyBiographyEvidence {
        &self.evidence
    }

    pub fn encoded_evidence(&self) -> &[u8] {
        &self.encoded
    }

    /// Publish an exact lifecycle extension without dropping workload or
    /// membership evidence. A failed append or encoded-size check is atomic.
    pub fn retain_wake(
        &mut self,
        body: conduit_body::Body,
        wake: conduit_body::Wake,
        first_sequence: u64,
    ) -> Result<(), BodyWorkloadError> {
        let mut next = self.evidence.clone();
        next.append_wake(body, wake, first_sequence)
            .map_err(BodyWorkloadError::Biography)?;
        let encoded = serde_json::to_vec(&next).map_err(|_| BodyWorkloadError::EvidenceEncoding)?;
        if encoded.len() > MAX_BODY_EVIDENCE_BYTES {
            return Err(BodyWorkloadError::EvidenceTooLarge);
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
    ) -> Result<BodyWorkloadChange, BodyWorkloadError> {
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
    ) -> Result<BodyWorkloadChange, BodyWorkloadError> {
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
    ) -> Result<BodyWorkloadChange, BodyWorkloadError> {
        let current = self.evidence.body.workload_revision;
        if expected_workload_revision != current {
            return Err(BodyWorkloadError::StaleWorkloadRevision {
                current,
                offered: expected_workload_revision,
            });
        }
        if self.evidence.body.state != BodyState::Lulled {
            return Err(BodyWorkloadError::BodyAwake);
        }

        let next_body = match kind {
            BodyWorkloadChangeKind::Admitted => {
                self.evidence.body.admit_form(form.clone(), sign_id.clone())
            }
            BodyWorkloadChangeKind::Removed => {
                self.evidence.body.remove_form(&form, sign_id.clone())
            }
        }
        .map_err(BodyWorkloadError::Lifecycle)?;
        let mut next_evidence = self.evidence.clone();
        next_evidence
            .append_body_workload_events(next_body, &[(sign_id.clone(), biography_sequence)])
            .map_err(BodyWorkloadError::Biography)?;
        let next_encoded =
            serde_json::to_vec(&next_evidence).map_err(|_| BodyWorkloadError::EvidenceEncoding)?;
        if next_encoded.len() > MAX_BODY_EVIDENCE_BYTES {
            return Err(BodyWorkloadError::EvidenceTooLarge);
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
