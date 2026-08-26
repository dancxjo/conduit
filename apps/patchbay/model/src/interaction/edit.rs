//! Exact typed authoring edits above renderer-local input mechanics.

use conduit_core::{ConfigurationValue, ExpandedFormId, SourceDocumentId};

use super::{codec::validate_field, InteractionError, PatchbayInteractionRequestId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayEditBasis {
    pub source_document_id: SourceDocumentId,
    pub source_revision: u64,
    pub expanded_form_id: ExpandedFormId,
}

impl PatchbayEditBasis {
    pub fn new(
        source_document_id: SourceDocumentId,
        source_revision: u64,
        expanded_form_id: ExpandedFormId,
    ) -> Result<Self, InteractionError> {
        validate_field(source_document_id.as_str())?;
        validate_field(expanded_form_id.as_str())?;
        Ok(Self {
            source_document_id,
            source_revision,
            expanded_form_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayEdit {
    PlaceGear {
        basis: PatchbayEditBasis,
        kind_id: String,
    },
    DuplicateGear {
        basis: PatchbayEditBasis,
        subject_identity: String,
    },
    RemoveGear {
        basis: PatchbayEditBasis,
        subject_identity: String,
    },
    RemoveCord {
        basis: PatchbayEditBasis,
        subject_identity: String,
    },
    ConnectPorts {
        basis: PatchbayEditBasis,
        source_identity: String,
        sink_identity: String,
    },
    RerouteCord {
        basis: PatchbayEditBasis,
        cord_identity: String,
        endpoint_identity: String,
    },
    ConfigureGear {
        basis: PatchbayEditBasis,
        subject_identity: String,
        key: String,
        value: ConfigurationValue,
    },
}

impl PatchbayEdit {
    pub fn basis(&self) -> &PatchbayEditBasis {
        match self {
            Self::PlaceGear { basis, .. }
            | Self::DuplicateGear { basis, .. }
            | Self::RemoveGear { basis, .. }
            | Self::RemoveCord { basis, .. }
            | Self::ConnectPorts { basis, .. }
            | Self::RerouteCord { basis, .. }
            | Self::ConfigureGear { basis, .. } => basis,
        }
    }

    pub const fn operation(&self) -> &'static str {
        match self {
            Self::PlaceGear { .. } => "place-gear",
            Self::DuplicateGear { .. } => "duplicate-gear",
            Self::RemoveGear { .. } => "remove-gear",
            Self::RemoveCord { .. } => "remove-cord",
            Self::ConnectPorts { .. } => "connect-ports",
            Self::RerouteCord { .. } => "reroute-cord",
            Self::ConfigureGear { .. } => "configure-gear",
        }
    }

    pub(super) fn validate(&self) -> Result<(), InteractionError> {
        PatchbayEditBasis::new(
            self.basis().source_document_id.clone(),
            self.basis().source_revision,
            self.basis().expanded_form_id.clone(),
        )?;
        match self {
            Self::PlaceGear { kind_id, .. } => validate_field(kind_id),
            Self::DuplicateGear {
                subject_identity, ..
            }
            | Self::RemoveGear {
                subject_identity, ..
            }
            | Self::RemoveCord {
                subject_identity, ..
            } => validate_field(subject_identity),
            Self::ConnectPorts {
                source_identity,
                sink_identity,
                ..
            } => {
                validate_field(source_identity)?;
                validate_field(sink_identity)
            }
            Self::RerouteCord {
                cord_identity,
                endpoint_identity,
                ..
            } => {
                validate_field(cord_identity)?;
                validate_field(endpoint_identity)
            }
            Self::ConfigureGear {
                subject_identity,
                key,
                value,
                ..
            } => {
                validate_field(subject_identity)?;
                validate_field(key)?;
                if let ConfigurationValue::Text(text) = value {
                    if text.len() > super::MAX_INTERACTION_ID_BYTES {
                        return Err(InteractionError::ValueTooLarge);
                    }
                }
                if matches!(value, ConfigurationValue::Structured(_)) {
                    return Err(InteractionError::UnsupportedConfiguration);
                }
                Ok(())
            }
        }
    }
}

pub(super) fn request_id_and_edit(
    request_id: PatchbayInteractionRequestId,
    edit: PatchbayEdit,
) -> Result<(PatchbayInteractionRequestId, PatchbayEdit), InteractionError> {
    edit.validate()?;
    Ok((request_id, edit))
}
