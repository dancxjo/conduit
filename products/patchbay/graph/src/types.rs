//! Finite typed graph facts for graphical Patchbay renderers.

use crate::prelude::*;

use conduit_core::{
    CheckedFormId, ExpandedFormId, GearId, KindContractRevision, KindId, PortDescriptor,
    PortTemporal, SourceDocumentId,
};

pub const MAX_PATCHBAY_GEARS: usize = 128;
pub const MAX_PATCHBAY_PORTS: usize = 512;
pub const MAX_PATCHBAY_CORDS: usize = 512;
pub const MAX_PATCHBAY_SUBJECTS: usize =
    MAX_PATCHBAY_GEARS + MAX_PATCHBAY_PORTS + MAX_PATCHBAY_CORDS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayGraphError {
    TooManyGears,
    TooManyPorts,
    TooManyCords,
    TooManySubjects,
    TooManyControls,
    InvalidConfigurationContract,
    MissingCordEndpoint,
    CordContractMismatch,
    StaleGraphBasis,
    UnknownSubject,
}

impl core::fmt::Display for PatchbayGraphError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let message = match self {
            Self::TooManyGears => "Patchbay graph exceeds its finite Gear bound",
            Self::TooManyPorts => "Patchbay graph exceeds its finite Port bound",
            Self::TooManyCords => "Patchbay graph exceeds its finite Cord bound",
            Self::TooManySubjects => "Patchbay graph exceeds its finite subject bound",
            Self::TooManyControls => "Patchbay Gear exceeds its finite Face-control bound",
            Self::InvalidConfigurationContract => {
                "Patchbay Gear configuration differs from its authoritative Kind contract"
            }
            Self::MissingCordEndpoint => "Patchbay Cord does not name two admitted exact Ports",
            Self::CordContractMismatch => {
                "Patchbay Cord Info or temporal contract differs from its exact Ports"
            }
            Self::StaleGraphBasis => "Patchbay selection candidate names a stale expanded Form",
            Self::UnknownSubject => "Patchbay inspector subject is not in the typed graph",
        };
        formatter.write_str(message)
    }
}

impl core::error::Error for PatchbayGraphError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchbaySubjectKind {
    Gear,
    Composition,
    FaceInput,
    FaceOutput,
    PortInput,
    PortOutput,
    Cord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayPortCompatibility {
    Compatible,
    DuplicateCord,
    UnknownPort,
    InvalidDirection,
    IncompatibleInfo {
        source: KindId,
        sink: KindId,
    },
    IncompatibleTemporal {
        source: PortTemporal,
        sink: PortTemporal,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayConnectionCandidate {
    pub sink_identity: String,
    pub compatibility: PatchbayPortCompatibility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayPort {
    pub identity: String,
    pub gear_id: GearId,
    pub descriptor: PortDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayFacePort {
    pub identity: String,
    pub descriptor: PortDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayCompositionBinding {
    pub face_port: String,
    pub internal_port: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayComposition {
    pub identity: String,
    pub gear_name: String,
    pub back_name: String,
    pub checked_form_id: CheckedFormId,
    pub inputs: Vec<PatchbayFacePort>,
    pub outputs: Vec<PatchbayFacePort>,
    pub input_bindings: Vec<PatchbayCompositionBinding>,
    pub output_bindings: Vec<PatchbayCompositionBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayGear {
    pub identity: String,
    pub gear_id: GearId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub source_form: String,
    pub form_path: Vec<String>,
    pub inputs: Vec<PatchbayPort>,
    pub outputs: Vec<PatchbayPort>,
    /// Direct, finite controls projected from the exact checked configuration
    /// and its authoritative Kind contract. This is never a configuration store.
    pub controls: Vec<crate::FaceControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayCord {
    pub identity: String,
    pub source_port: String,
    pub sink_port: String,
    pub value_kind: KindId,
    pub temporal: PortTemporal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayGraph {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub form_name: String,
    pub face_inputs: Vec<PatchbayFacePort>,
    pub face_outputs: Vec<PatchbayFacePort>,
    pub compositions: Vec<PatchbayComposition>,
    pub gears: Vec<PatchbayGear>,
    pub cords: Vec<PatchbayCord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayInspection {
    pub subject_identity: String,
    pub subject_kind: PatchbaySubjectKind,
    pub exact_facts: Vec<String>,
}

/// Exact pre-admission subject resolved from renderer-local geometry.
///
/// This contains no coordinates or platform identity. Binding the subject to its expanded Form
/// prevents a retained hit target from being applied to a replacement projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbaySubjectRef {
    pub expanded_form_id: ExpandedFormId,
    pub subject_identity: String,
}
