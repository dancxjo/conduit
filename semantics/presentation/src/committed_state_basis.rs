//! Finite provenance vocabulary for a checkpoint-backed Face contribution.
//!
//! This module checks identity coherence only. It does not authenticate a Host
//! Call, a terminal Sign, or checkpoint bytes. In particular, a valid basis is
//! not by itself admissible to `Face::project`; the Owner must first provide an
//! independently verified read of the selected generation.

use conduit_body::{Body, BodyId, BodyState};
use conduit_core::{
    ActivePlayId, CheckedPlotId, PlanId, ResourceSemanticIdentity, ResourceVersionIdentity, SignId,
};

/// Exact selected checkpoint and current published generation. These are
/// semantic identities, not a path, handle, or authority to open the resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedStateSelection {
    pub resource: ResourceSemanticIdentity,
    pub selected_version: ResourceVersionIdentity,
    pub published_version: ResourceVersionIdentity,
}

/// Correlation carried by a future authenticated Owner admission receipt.
/// A write has finished before the read verifies this exact generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedStateOperation {
    pub plan_id: PlanId,
    pub play_id: ActivePlayId,
    pub terminal_sign_id: SignId,
}

/// Distinct from a current-Playing `PresentationContributionBasis`. The digest
/// covers the state value from the admitted read, not arbitrary rendered text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedStateContributionBasis {
    pub body_id: BodyId,
    pub checked_plot_id: CheckedPlotId,
    pub selection: CommittedStateSelection,
    pub write: CommittedStateOperation,
    pub read: CommittedStateOperation,
    pub state_digest: [u8; 32],
}

/// Shape and current-residence refusals; they are not Host Call verdicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommittedStateBasisRefusal {
    BodyChanged,
    PlotNotResident,
    BodyStillAwake,
    CheckpointSelectionChanged,
    PublishedVersionChanged,
    MissingResourceIdentity,
    MissingSelectedVersion,
    MissingPublishedVersion,
    MissingStateDigest,
    InvalidOperationIdentity,
    ReusedOperation,
}

/// Decisions reserved for the Owner's authenticated Host Call/Sign verifier.
/// Shape validation cannot emit these: a string matching a Sign ID is not
/// evidence that the Sign exists or reports successful publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommittedStateEvidenceRefusal {
    MissingWriteReceipt,
    MissingReadReceipt,
    MissingTerminalSign,
    FailedTerminalSign,
    ReadVersionMismatch,
    ReadDigestMismatch,
    CorruptCheckpoint,
    BootChanged,
}

const MAX_OPERATION_ID_BYTES: usize = 128;

impl CommittedStateOperation {
    fn has_finite_identities(&self) -> bool {
        [
            self.plan_id.as_str(),
            self.play_id.as_str(),
            self.terminal_sign_id.as_str(),
        ]
        .iter()
        .all(|identity| !identity.is_empty() && identity.len() <= MAX_OPERATION_ID_BYTES)
    }
}

impl CommittedStateContributionBasis {
    /// Check finite shape and the currently selected Body/Plot/resource facts.
    /// This is deliberately insufficient for Face admission: the caller must
    /// obtain authenticated read/write Host Call and Sign evidence separately.
    pub fn validate_shape_against(
        &self,
        body: &Body,
        selection: &CommittedStateSelection,
    ) -> Result<(), CommittedStateBasisRefusal> {
        if self.body_id != body.body_id {
            return Err(CommittedStateBasisRefusal::BodyChanged);
        }
        if !body
            .workset
            .plots()
            .iter()
            .any(|plot| plot.checked_plot_id == self.checked_plot_id)
        {
            return Err(CommittedStateBasisRefusal::PlotNotResident);
        }
        if matches!(body.state, BodyState::Awake { .. }) {
            return Err(CommittedStateBasisRefusal::BodyStillAwake);
        }
        if self.selection.resource.digest() == [0; 32] {
            return Err(CommittedStateBasisRefusal::MissingResourceIdentity);
        }
        if self.selection.selected_version.digest() == [0; 32] {
            return Err(CommittedStateBasisRefusal::MissingSelectedVersion);
        }
        if self.selection.published_version.digest() == [0; 32] {
            return Err(CommittedStateBasisRefusal::MissingPublishedVersion);
        }
        if self.selection.resource != selection.resource
            || self.selection.selected_version != selection.selected_version
        {
            return Err(CommittedStateBasisRefusal::CheckpointSelectionChanged);
        }
        if self.selection.published_version != selection.published_version {
            return Err(CommittedStateBasisRefusal::PublishedVersionChanged);
        }
        if self.state_digest == [0; 32] {
            return Err(CommittedStateBasisRefusal::MissingStateDigest);
        }
        if !self.write.has_finite_identities() || !self.read.has_finite_identities() {
            return Err(CommittedStateBasisRefusal::InvalidOperationIdentity);
        }
        if self.write.plan_id == self.read.plan_id
            || self.write.play_id == self.read.play_id
            || self.write.terminal_sign_id == self.read.terminal_sign_id
        {
            return Err(CommittedStateBasisRefusal::ReusedOperation);
        }
        Ok(())
    }
}
