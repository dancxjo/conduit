//! Finite typed graph facts for graphical Patchbay renderers.

use conduit_core::{
    CheckedFormId, ExpandedFormId, GearId, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal, SourceDocumentId,
};
use conduit_form::ExpandedCanonicalForm;

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

impl std::fmt::Display for PatchbayGraphError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl std::error::Error for PatchbayGraphError {}

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

impl PatchbayGraph {
    pub fn subject_count(&self) -> usize {
        self.gears.len()
            + self.compositions.len()
            + self.face_inputs.len()
            + self.face_outputs.len()
            + self
                .gears
                .iter()
                .map(|gear| gear.inputs.len() + gear.outputs.len())
                .sum::<usize>()
            + self
                .compositions
                .iter()
                .map(|composition| composition.inputs.len() + composition.outputs.len())
                .sum::<usize>()
            + self.cords.len()
    }

    pub fn admit_composition(
        &mut self,
        composition: PatchbayComposition,
    ) -> Result<(), PatchbayGraphError> {
        let gear_count = self
            .gears
            .len()
            .checked_add(self.compositions.len())
            .and_then(|count| count.checked_add(1));
        if gear_count.is_none_or(|count| count > MAX_PATCHBAY_GEARS) {
            return Err(PatchbayGraphError::TooManyGears);
        }
        let existing_ports = self.face_inputs.len()
            + self.face_outputs.len()
            + self
                .gears
                .iter()
                .map(|gear| gear.inputs.len() + gear.outputs.len())
                .sum::<usize>()
            + self
                .compositions
                .iter()
                .map(|candidate| candidate.inputs.len() + candidate.outputs.len())
                .sum::<usize>();
        let port_count = existing_ports
            .checked_add(composition.inputs.len())
            .and_then(|count| count.checked_add(composition.outputs.len()));
        if port_count.is_none_or(|count| count > MAX_PATCHBAY_PORTS) {
            return Err(PatchbayGraphError::TooManyPorts);
        }
        let subject_count = gear_count
            .and_then(|gears| port_count.and_then(|ports| gears.checked_add(ports)))
            .and_then(|count| count.checked_add(self.cords.len()));
        if subject_count.is_none_or(|count| count > MAX_PATCHBAY_SUBJECTS) {
            return Err(PatchbayGraphError::TooManySubjects);
        }
        self.compositions.push(composition);
        Ok(())
    }

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
                let provenance = form
                    .provenance
                    .iter()
                    .find(|candidate| candidate.gear_id == gear.gear_id.as_str())
                    .ok_or(PatchbayGraphError::UnknownSubject)?;
                Ok(PatchbayGear {
                    identity: format!("gear/{}", gear.gear_id.as_str()),
                    gear_id: gear.gear_id.clone(),
                    kind_id: gear.kind_id.clone(),
                    kind_contract_revision: gear.kind_contract_revision.clone(),
                    source_form: provenance.source_form.clone(),
                    form_path: provenance.form_path.clone(),
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
            face_inputs: Vec::new(),
            face_outputs: Vec::new(),
            compositions: Vec::new(),
            gears,
            cords,
        })
    }

    pub fn from_authoring(
        form: &conduit_form::ExpandedAuthoringForm,
    ) -> Result<Self, PatchbayGraphError> {
        let mut graph = Self::from_expanded(&form.expanded)?;
        let boundary_count = form.face.inputs().len() + form.face.outputs().len();
        let port_count = graph
            .gears
            .iter()
            .map(|gear| gear.inputs.len() + gear.outputs.len())
            .sum::<usize>();
        if port_count
            .checked_add(boundary_count)
            .is_none_or(|count| count > MAX_PATCHBAY_PORTS)
        {
            return Err(PatchbayGraphError::TooManyPorts);
        }
        graph.face_inputs = form
            .face
            .inputs()
            .iter()
            .cloned()
            .map(|descriptor| PatchbayFacePort {
                identity: face_port_identity(PortDirection::Input, descriptor.port_id.as_str()),
                descriptor,
            })
            .collect();
        graph.face_outputs = form
            .face
            .outputs()
            .iter()
            .cloned()
            .map(|descriptor| PatchbayFacePort {
                identity: face_port_identity(PortDirection::Output, descriptor.port_id.as_str()),
                descriptor,
            })
            .collect();
        let boundary_cords = form.input_bindings.len() + form.output_bindings.len();
        if graph
            .cords
            .len()
            .checked_add(boundary_cords)
            .is_none_or(|count| count > MAX_PATCHBAY_CORDS)
        {
            return Err(PatchbayGraphError::TooManyCords);
        }
        for binding in &form.input_bindings {
            let source = face_port_identity(PortDirection::Input, binding.face_port_id.as_str());
            let sink = port_identity(
                &binding.gear_id,
                PortDirection::Input,
                binding.gear_port_id.as_str(),
            );
            let descriptor = graph
                .face_inputs
                .iter()
                .find(|port| port.identity == source)
                .ok_or(PatchbayGraphError::MissingCordEndpoint)?
                .descriptor
                .clone();
            graph.cords.push(PatchbayCord {
                identity: format!("boundary/{source}->{sink}"),
                source_port: source,
                sink_port: sink,
                value_kind: descriptor.value_kind,
                temporal: descriptor.temporal,
            });
        }
        for binding in &form.output_bindings {
            let source = port_identity(
                &binding.gear_id,
                PortDirection::Output,
                binding.gear_port_id.as_str(),
            );
            let sink = face_port_identity(PortDirection::Output, binding.face_port_id.as_str());
            let descriptor = graph
                .face_outputs
                .iter()
                .find(|port| port.identity == sink)
                .ok_or(PatchbayGraphError::MissingCordEndpoint)?
                .descriptor
                .clone();
            graph.cords.push(PatchbayCord {
                identity: format!("boundary/{source}->{sink}"),
                source_port: source,
                sink_port: sink,
                value_kind: descriptor.value_kind,
                temporal: descriptor.temporal,
            });
        }
        Ok(graph)
    }

    pub fn subject_identities(&self) -> impl Iterator<Item = &str> {
        self.face_inputs
            .iter()
            .chain(&self.face_outputs)
            .map(|port| port.identity.as_str())
            .chain(self.compositions.iter().flat_map(|composition| {
                std::iter::once(composition.identity.as_str())
                    .chain(composition.inputs.iter().map(|port| port.identity.as_str()))
                    .chain(
                        composition
                            .outputs
                            .iter()
                            .map(|port| port.identity.as_str()),
                    )
            }))
            .chain(self.gears.iter().flat_map(|gear| {
                std::iter::once(gear.identity.as_str())
                    .chain(gear.inputs.iter().map(|port| port.identity.as_str()))
                    .chain(gear.outputs.iter().map(|port| port.identity.as_str()))
            }))
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
        if let Some(composition) = self
            .compositions
            .iter()
            .find(|composition| composition.identity == identity)
        {
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind: PatchbaySubjectKind::Composition,
                exact_facts: vec![
                    format!("Gear {}", composition.gear_name),
                    format!("Back {}", composition.back_name),
                    format!("checked {}", composition.checked_form_id.as_str()),
                    format!(
                        "inputs={} outputs={}",
                        composition.inputs.len(),
                        composition.outputs.len()
                    ),
                ],
            });
        }
        if let Some((composition, port)) = self.compositions.iter().find_map(|composition| {
            composition
                .inputs
                .iter()
                .chain(&composition.outputs)
                .find(|port| port.identity == identity)
                .map(|port| (composition, port))
        }) {
            let subject_kind = match port.descriptor.direction {
                PortDirection::Input => PatchbaySubjectKind::PortInput,
                PortDirection::Output => PatchbaySubjectKind::PortOutput,
            };
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind,
                exact_facts: vec![
                    format!("Composition {}", composition.gear_name),
                    format!("Back {}", composition.back_name),
                    format!("Port {}", port.descriptor.port_id.as_str()),
                    format!("Info {}", port.descriptor.value_kind.as_str()),
                    format!("temporal={:?}", port.descriptor.temporal),
                ],
            });
        }
        if let Some(port) = self
            .face_inputs
            .iter()
            .chain(&self.face_outputs)
            .find(|port| port.identity == identity)
        {
            let subject_kind = match port.descriptor.direction {
                PortDirection::Input => PatchbaySubjectKind::FaceInput,
                PortDirection::Output => PatchbaySubjectKind::FaceOutput,
            };
            return Ok(PatchbayInspection {
                subject_identity: identity.into(),
                subject_kind,
                exact_facts: vec![
                    format!("Face Port {}", port.descriptor.port_id.as_str()),
                    format!("direction={:?}", port.descriptor.direction),
                    format!("Info {}", port.descriptor.value_kind.as_str()),
                    format!("temporal={:?}", port.descriptor.temporal),
                    "authoring boundary; runnable root requires an exact binding".into(),
                ],
            });
        }
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

fn face_port_identity(direction: PortDirection, port: &str) -> String {
    let direction = match direction {
        PortDirection::Input => "input",
        PortDirection::Output => "output",
    };
    format!("face/{direction}/{port}")
}
