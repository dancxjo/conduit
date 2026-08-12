use crate::transport_types::RendererSnapshot;
use conduit_core::SignId;
use conduit_presentation::{ManifestationFailure, ManifestationLifecycle};
use patchbay_model::RendererExecution;

pub const SNAPSHOT_SCHEMA: &str = "conduit.patchbay.portable-presentation";
pub const MAX_SNAPSHOT_BYTES: usize = conduit_presentation::MAX_PRESENTATION_TOTAL_BYTES + 16_384;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    Oversized,
    Malformed(String),
    UnsupportedSchema,
    Stale { minimum: u64, offered: u64 },
    InvalidIdentity,
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Oversized => {
                formatter.write_str("renderer snapshot exceeds its finite byte bound")
            }
            Self::Malformed(message) => write!(formatter, "malformed renderer snapshot: {message}"),
            Self::UnsupportedSchema => formatter.write_str("unsupported snapshot schema"),
            Self::Stale { minimum, offered } => {
                write!(
                    formatter,
                    "stale renderer revision {offered}; minimum is {minimum}"
                )
            }
            Self::InvalidIdentity => {
                formatter.write_str("portable Presentation identity is invalid")
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

impl RendererSnapshot {
    pub fn from_execution(execution: RendererExecution) -> Result<Self, SnapshotError> {
        execution
            .validate()
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        let entrance = patchbay_model::PatchbayEntranceState::enter(&execution.presentation)
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        let value = Self {
            schema: SNAPSHOT_SCHEMA.into(),
            revision: execution.presentation.revision,
            renderer: execution
                .self_inspection()
                .map_err(|_| SnapshotError::InvalidIdentity)?,
            presentation: execution.presentation,
            entrance,
            parts: None,
            interaction: crate::HtmlInteractionState::default(),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn mark_available(&mut self, sign_id: SignId) -> Result<(), SnapshotError> {
        self.renderer.manifestation = self
            .renderer
            .manifestation
            .transition(ManifestationLifecycle::Available, sign_id)
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        self.validate()
    }

    pub fn attach_parts(&mut self, parts: patchbay_model::PartsView) -> Result<(), SnapshotError> {
        if parts.body_id != self.presentation.basis.body_id {
            return Err(SnapshotError::InvalidIdentity);
        }
        self.parts = Some(parts);
        self.validate()
    }

    pub fn mark_failed(
        &mut self,
        failure: ManifestationFailure,
        sign_id: SignId,
    ) -> Result<(), SnapshotError> {
        self.renderer.manifestation = self
            .renderer
            .manifestation
            .fail(failure, sign_id)
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        self.validate()
    }

    pub fn mark_closed(&mut self, sign_id: SignId) -> Result<(), SnapshotError> {
        self.renderer.manifestation = self
            .renderer
            .manifestation
            .transition(ManifestationLifecycle::Closed, sign_id)
            .map_err(|_| SnapshotError::InvalidIdentity)?;
        self.validate()
    }

    pub fn encode(&self) -> Result<Vec<u8>, SnapshotError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| SnapshotError::Malformed(error.to_string()))?;
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(SnapshotError::Oversized);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8], minimum_revision: u64) -> Result<Self, SnapshotError> {
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(SnapshotError::Oversized);
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| SnapshotError::Malformed(error.to_string()))?;
        if value.revision < minimum_revision {
            return Err(SnapshotError::Stale {
                minimum: minimum_revision,
                offered: value.revision,
            });
        }
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), SnapshotError> {
        let invalid_parts = self.parts.as_ref().is_some_and(|parts| {
            parts.body_id != self.presentation.basis.body_id
                || parts.parts.len() > patchbay_model::MAX_PARTS_VIEW_ROWS
                || parts.wants_to_join.len() > patchbay_model::MAX_WANTS_TO_JOIN_ROWS
        });
        if self.schema != SNAPSHOT_SCHEMA {
            return Err(SnapshotError::UnsupportedSchema);
        }
        if self.revision != self.presentation.revision
            || self.presentation.validate().is_err()
            || self.renderer.validate_against(&self.presentation).is_err()
            || self.entrance.body_id != self.presentation.basis.body_id
            || self.entrance.presentation_id != self.presentation.identity.as_str()
            || self.entrance.presentation_revision != self.presentation.revision
            || invalid_parts
        {
            return Err(SnapshotError::InvalidIdentity);
        }
        Ok(())
    }
}
