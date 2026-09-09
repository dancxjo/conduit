//! Projection from exact checked and expanded Form truth.
use crate::graph::temporal_compatible;
use crate::{prelude::*, *};
use conduit_core::{GearId, PortDescriptor, PortDirection};
use conduit_form::ExpandedCanonicalForm;

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
                || !temporal_compatible(connection.temporal, sink_port.descriptor.temporal)
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
