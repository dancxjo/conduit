//! Finite typed graph facts for graphical Patchbay renderers.

use conduit_core::{
    CheckedFormId, ExpandedFormId, GearId, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal, SourceDocumentId,
};
use conduit_form::ExpandedCanonicalForm;

pub const MAX_PATCHBAY_GEARS: usize = 128;
pub const MAX_PATCHBAY_PORTS: usize = 512;
pub const MAX_PATCHBAY_CORDS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayGraphError {
    TooManyGears,
    TooManyPorts,
    TooManyCords,
    TooManyControls,
    InvalidConfigurationContract,
    MissingCordEndpoint,
    CordContractMismatch,
    StaleGraphBasis,
    UnknownSubject,
}

impl std::fmt::Display for PatchbayGraphError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::TooManyGears => "Patchbay graph exceeds its finite Gear bound",
            Self::TooManyPorts => "Patchbay graph exceeds its finite Port bound",
            Self::TooManyCords => "Patchbay graph exceeds its finite Cord bound",
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

impl std::error::Error for PatchbayGraphError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchbaySubjectKind {
    Gear,
    PortInput,
    PortOutput,
    Cord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayPort {
    pub identity: String,
    pub gear_id: GearId,
    pub descriptor: PortDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayGear {
    pub identity: String,
    pub gear_id: GearId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
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

impl PatchbayGraph {
    pub fn from_expanded(form: &ExpandedCanonicalForm) -> Result<Self, PatchbayGraphError> {
        if form.gears.len() > MAX_PATCHBAY_GEARS {
            return Err(PatchbayGraphError::TooManyGears);
        }
        if form.connections.len() > MAX_PATCHBAY_CORDS {
            return Err(PatchbayGraphError::TooManyCords);
        }
        let port_count = form.gears.iter().try_fold(0usize, |count, gear| {
            count
                .checked_add(gear.inputs.len())?
                .checked_add(gear.outputs.len())
        });
        if port_count.is_none_or(|count| count > MAX_PATCHBAY_PORTS) {
            return Err(PatchbayGraphError::TooManyPorts);
        }
        let gears = form
            .gears
            .iter()
            .map(|gear| {
                Ok(PatchbayGear {
                    identity: format!("gear/{}", gear.gear_id.as_str()),
                    gear_id: gear.gear_id.clone(),
                    kind_id: gear.kind_id.clone(),
                    kind_contract_revision: gear.kind_contract_revision.clone(),
                    inputs: gear
                        .inputs
                        .iter()
                        .map(|port| patchbay_port(&gear.gear_id, port))
                        .collect(),
                    outputs: gear
                        .outputs
                        .iter()
                        .map(|port| patchbay_port(&gear.gear_id, port))
                        .collect(),
                    controls: crate::face_controls::project_controls(gear)?,
                })
            })
            .collect::<Result<Vec<_>, PatchbayGraphError>>()?;
        let mut cords = Vec::with_capacity(form.connections.len());
        for (index, connection) in form.connections.iter().enumerate() {
            let source = port_identity(
                &connection.source_gear_id,
                PortDirection::Output,
                connection.source_port_id.as_str(),
            );
            let sink = port_identity(
                &connection.sink_gear_id,
                PortDirection::Input,
                connection.sink_port_id.as_str(),
            );
            let source_port = gears
                .iter()
                .flat_map(|gear| &gear.outputs)
                .find(|port| port.identity == source);
            let sink_port = gears
                .iter()
                .flat_map(|gear| &gear.inputs)
                .find(|port| port.identity == sink);
            let (Some(source_port), Some(sink_port)) = (source_port, sink_port) else {
                return Err(PatchbayGraphError::MissingCordEndpoint);
            };
            if source_port.descriptor.value_kind != connection.value_kind
                || sink_port.descriptor.value_kind != connection.value_kind
                || source_port.descriptor.temporal != connection.temporal
                || sink_port.descriptor.temporal != connection.temporal
            {
                return Err(PatchbayGraphError::CordContractMismatch);
            }
            cords.push(PatchbayCord {
                identity: format!("cord/{index}/{source}->{sink}"),
                source_port: source,
                sink_port: sink,
                value_kind: connection.value_kind.clone(),
                temporal: connection.temporal,
            });
        }
        Ok(Self {
            source_document_id: form.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
            expanded_form_id: form.expanded_form_id.clone(),
            form_name: form.name.clone(),
            gears,
            cords,
        })
    }

    pub fn subject_identities(&self) -> impl Iterator<Item = &str> {
        self.gears
            .iter()
            .flat_map(|gear| {
                std::iter::once(gear.identity.as_str())
                    .chain(gear.inputs.iter().map(|port| port.identity.as_str()))
                    .chain(gear.outputs.iter().map(|port| port.identity.as_str()))
            })
            .chain(self.cords.iter().map(|cord| cord.identity.as_str()))
    }

    pub fn subject_ref(&self, identity: &str) -> Result<PatchbaySubjectRef, PatchbayGraphError> {
        self.subject_index(identity)?;
        Ok(PatchbaySubjectRef {
            expanded_form_id: self.expanded_form_id.clone(),
            subject_identity: identity.into(),
        })
    }

    pub fn resolve_subject_ref(
        &self,
        subject: &PatchbaySubjectRef,
    ) -> Result<usize, PatchbayGraphError> {
        if subject.expanded_form_id != self.expanded_form_id {
            return Err(PatchbayGraphError::StaleGraphBasis);
        }
        self.subject_index(&subject.subject_identity)
    }

    pub fn inspect(&self, identity: &str) -> Result<PatchbayInspection, PatchbayGraphError> {
        if let Some(gear) = self.gears.iter().find(|gear| gear.identity == identity) {
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind: PatchbaySubjectKind::Gear,
                exact_facts: vec![
                    format!("Gear {}", gear.gear_id.as_str()),
                    format!("Kind {}", gear.kind_id.as_str()),
                    format!(
                        "inputs={} outputs={}",
                        gear.inputs.len(),
                        gear.outputs.len()
                    ),
                ],
            });
        }
        if let Some(port) = self
            .gears
            .iter()
            .flat_map(|gear| gear.inputs.iter().chain(&gear.outputs))
            .find(|port| port.identity == identity)
        {
            let subject_kind = match port.descriptor.direction {
                PortDirection::Input => PatchbaySubjectKind::PortInput,
                PortDirection::Output => PatchbaySubjectKind::PortOutput,
            };
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind,
                exact_facts: vec![
                    format!("Gear {}", port.gear_id.as_str()),
                    format!("Port {}", port.descriptor.port_id.as_str()),
                    format!("Info {}", port.descriptor.value_kind.as_str()),
                    format!("temporal={:?}", port.descriptor.temporal),
                ],
            });
        }
        if let Some(cord) = self.cords.iter().find(|cord| cord.identity == identity) {
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind: PatchbaySubjectKind::Cord,
                exact_facts: vec![
                    format!("from {}", cord.source_port),
                    format!("to {}", cord.sink_port),
                    format!("Info {}", cord.value_kind.as_str()),
                    format!("temporal={:?}", cord.temporal),
                    "semantic parameters: none exposed by this Cord contract".into(),
                    "Line / transport choices belong to realization".into(),
                ],
            });
        }
        Err(PatchbayGraphError::UnknownSubject)
    }

    fn subject_index(&self, identity: &str) -> Result<usize, PatchbayGraphError> {
        self.subject_identities()
            .position(|candidate| candidate == identity)
            .ok_or(PatchbayGraphError::UnknownSubject)
    }
}

fn patchbay_port(gear_id: &GearId, descriptor: &PortDescriptor) -> PatchbayPort {
    PatchbayPort {
        identity: port_identity(gear_id, descriptor.direction, descriptor.port_id.as_str()),
        gear_id: gear_id.clone(),
        descriptor: descriptor.clone(),
    }
}

fn port_identity(gear: &GearId, direction: PortDirection, port: &str) -> String {
    let direction = match direction {
        PortDirection::Input => "input",
        PortDirection::Output => "output",
    };
    format!("port/{}/{direction}/{port}", gear.as_str())
}
