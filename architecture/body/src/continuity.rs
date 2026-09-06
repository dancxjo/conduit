//! Continuous Form execution over finitely admitted Play work.
//!
//! Continuous execution is a lifetime property, not an unbounded allocation
//! or a hidden restart loop.  The contract below is deliberately small: a
//! retained finite value may accept an arbitrary number of externally driven
//! transitions, while every individual Play admits the storage and operation
//! slots it needs before it starts.

use conduit_core::{CheckedFormId, PlanId, SourceDocumentId};
use serde::{Deserialize, Serialize};

/// Machine-readable outcomes for a continuing Form or one of its finite
/// interaction episodes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContinuousDisposition {
    /// The Form has completed its semantic work and will not continue.
    SemanticCompletion,
    /// The Form remains live but currently has no admitted work to perform.
    Quiescent,
    /// The Body deliberately ended the current Wake while retaining the Form.
    Lull,
    /// An explicit cancellation ended the current work.
    Cancelled,
    /// A finite typed value could not represent the next state.
    ValueOverflow,
    /// A finite queue, operation, or other admitted resource was exhausted.
    CapacityExhausted,
    /// The current work failed for a non-capacity reason.
    Failed,
    /// Current Host, Boot, resource, or Line truth was lost.
    HostBootResourceOrLineLost,
    /// The immutable realization is no longer current and must be replaced.
    PlanRetired,
    /// The same Form and retained state continued under a replacement Plan.
    Replanned,
    /// One finite transition was accepted and the Form remains live.
    Continued,
}

/// Finite storage and operation admission for one active Play.
///
/// These limits apply to the instantaneous workset.  They do not limit the
/// number of future transitions over the Form's lifetime.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuousResourceAdmission {
    pub retained_value_bytes: u16,
    pub queue_slots: u16,
    pub operation_slots: u16,
}

impl ContinuousResourceAdmission {
    pub const fn specimen() -> Self {
        Self {
            retained_value_bytes: 2,
            queue_slots: 1,
            operation_slots: 1,
        }
    }

    pub const fn is_finite_and_nonzero(self) -> bool {
        self.retained_value_bytes > 0 && self.queue_slots > 0 && self.operation_slots > 0
    }
}

/// A finite-state continuous specimen.
///
/// `state` and the admission are fixed-size.  `accept` can be called for as
/// many externally driven interactions as the caller supplies; no transition
/// counter or restart is part of the semantic contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuousSpecimen {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub plan_id: PlanId,
    pub state: u16,
    pub resources: ContinuousResourceAdmission,
    pub disposition: ContinuousDisposition,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ContinuousError {
    InvalidIdentity,
    InvalidAdmission,
    NotLive,
    SamePlan,
}

impl ContinuousSpecimen {
    pub fn admit(
        source_document_id: SourceDocumentId,
        checked_form_id: CheckedFormId,
        plan_id: PlanId,
        resources: ContinuousResourceAdmission,
    ) -> Result<Self, ContinuousError> {
        if source_document_id.as_str().is_empty()
            || checked_form_id.as_str().is_empty()
            || plan_id.as_str().is_empty()
        {
            return Err(ContinuousError::InvalidIdentity);
        }
        if !resources.is_finite_and_nonzero() {
            return Err(ContinuousError::InvalidAdmission);
        }
        Ok(Self {
            source_document_id,
            checked_form_id,
            plan_id,
            state: 0,
            resources,
            disposition: ContinuousDisposition::Quiescent,
        })
    }

    /// Accept one finite transition without allocating or renewing a Play.
    pub fn accept(&mut self, delta: u16) -> Result<ContinuousDisposition, ContinuousError> {
        if !matches!(
            self.disposition,
            ContinuousDisposition::Quiescent
                | ContinuousDisposition::Continued
                | ContinuousDisposition::Replanned
        ) {
            return Err(ContinuousError::NotLive);
        }
        self.state = match self.state.checked_add(delta) {
            Some(next) => next,
            None => {
                self.disposition = ContinuousDisposition::ValueOverflow;
                return Ok(self.disposition);
            }
        };
        self.disposition = ContinuousDisposition::Continued;
        Ok(self.disposition)
    }

    /// Replace only the realization.  Form identity and retained state stay
    /// unchanged, so this is not a semantic restart.
    pub fn replan(
        &mut self,
        replacement_plan_id: PlanId,
    ) -> Result<ContinuousDisposition, ContinuousError> {
        if replacement_plan_id.as_str().is_empty() {
            return Err(ContinuousError::InvalidIdentity);
        }
        if self.plan_id == replacement_plan_id {
            return Err(ContinuousError::SamePlan);
        }
        if !matches!(
            self.disposition,
            ContinuousDisposition::Quiescent
                | ContinuousDisposition::Continued
                | ContinuousDisposition::Replanned
        ) {
            return Err(ContinuousError::NotLive);
        }
        self.plan_id = replacement_plan_id;
        self.disposition = ContinuousDisposition::Replanned;
        Ok(self.disposition)
    }

    pub fn complete(&mut self) -> ContinuousDisposition {
        self.disposition = ContinuousDisposition::SemanticCompletion;
        self.disposition
    }
}
