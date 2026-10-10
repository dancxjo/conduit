//! Projection from exact checked and expanded Plot truth.
use crate::{prelude::*, *};
use conduit_core::{
    CheckedValueContract, FrontValueLocation, GearId, PortDescriptor, PortDirection,
};
use conduit_plot::ExpandedCanonicalPlot;

impl PatchbayGraph {
    pub fn from_expanded(plot: &ExpandedCanonicalPlot) -> Result<Self, PatchbayGraphError> {
        if plot.gears.len() > MAX_PATCHBAY_GEARS {
            return Err(PatchbayGraphError::TooManyGears);
        }
        if plot.connections.len() > MAX_PATCHBAY_CORDS {
            return Err(PatchbayGraphError::TooManyCords);
        }
        let port_count = plot.gears.iter().try_fold(0usize, |count, gear| {
            count
                .checked_add(gear.inputs.len())?
                .checked_add(gear.outputs.len())
        });
        if port_count.is_none_or(|count| count > MAX_PATCHBAY_PORTS) {
            return Err(PatchbayGraphError::TooManyPorts);
        }
        let gears = plot
            .gears
            .iter()
            .map(|gear| {
                let front = gear.checked_front();
                let provenance = plot
                    .provenance
                    .iter()
                    .find(|candidate| candidate.gear_id == gear.gear_id.as_str())
                    .ok_or(PatchbayGraphError::UnknownSubject)?;
                Ok(PatchbayGear {
                    identity: format!("gear/{}", gear.gear_id.as_str()),
                    gear_id: gear.gear_id.clone(),
                    kind_id: gear.kind_id.clone(),
                    kind_contract_revision: gear.kind_contract_revision.clone(),
                    source_plot: provenance.source_plot.clone(),
                    plot_path: provenance.plot_path.clone(),
                    inputs: gear
                        .inputs
                        .iter()
                        .map(|port| {
                            patchbay_port(
                                &gear.gear_id,
                                port,
                                front
                                    .value_contract(&FrontValueLocation::Input(
                                        port.port_id.clone(),
                                    ))
                                    .cloned(),
                            )
                        })
                        .collect(),
                    outputs: gear
                        .outputs
                        .iter()
                        .map(|port| {
                            patchbay_port(
                                &gear.gear_id,
                                port,
                                front
                                    .value_contract(&FrontValueLocation::Output(
                                        port.port_id.clone(),
                                    ))
                                    .cloned(),
                            )
                        })
                        .collect(),
                    controls: crate::front_controls::project_controls(gear)?,
                })
            })
            .collect::<Result<Vec<_>, PatchbayGraphError>>()?;
        let mut cords = Vec::with_capacity(plot.connections.len());
        for connection in &plot.connections {
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
            let expected_kind = match connection.track {
                conduit_core::ConnectionTrack::Payload => {
                    Some(source_port.descriptor.value_kind.clone())
                }
                conduit_core::ConnectionTrack::NormalClose
                | conduit_core::ConnectionTrack::Quiescence => {
                    Some(conduit_core::kind_id(conduit_core::EMPTY_INFO_ID))
                }
                conduit_core::ConnectionTrack::AbnormalTerminal => {
                    source_port.descriptor.abnormal_kind.clone()
                }
            };
            let expected_temporal = if connection.track == conduit_core::ConnectionTrack::Payload {
                source_port.descriptor.temporal
            } else {
                conduit_core::PortTemporal::Value
            };
            if expected_kind.as_ref() != Some(&connection.value_kind)
                || expected_temporal != connection.temporal
                || conduit_plot::validate_connection_contract(
                    &source_port.descriptor,
                    &sink_port.descriptor,
                    connection.track,
                )
                .is_err()
            {
                return Err(PatchbayGraphError::CordContractMismatch);
            }
            cords.push(PatchbayCord {
                // Length-prefix endpoints so delimiters in semantic names cannot
                // alias another tuple. Order in the expanded inventory is irrelevant.
                identity: format!(
                    "cord/{}/{}/{source}/{}/{sink}",
                    connection.track.as_str(),
                    source.len(),
                    sink.len(),
                ),
                source_port: source,
                sink_port: sink,
                value_kind: connection.value_kind.clone(),
                temporal: connection.temporal,
            });
        }
        Ok(Self {
            source_document_id: plot.source_document_id.clone(),
            checked_plot_id: plot.checked_plot_id.clone(),
            expanded_plot_id: plot.expanded_plot_id.clone(),
            plot_name: plot.name.clone(),
            front_inputs: Vec::new(),
            front_outputs: Vec::new(),
            compositions: Vec::new(),
            gears,
            cords,
        })
    }

    pub fn from_authoring(
        plot: &conduit_plot::ExpandedAuthoringPlot,
    ) -> Result<Self, PatchbayGraphError> {
        let mut graph = Self::from_expanded(&plot.expanded)?;
        let boundary_count = plot.front.inputs().len() + plot.front.outputs().len();
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
        graph.front_inputs = plot
            .front
            .inputs()
            .iter()
            .cloned()
            .map(|descriptor| PatchbayFrontPort {
                identity: front_port_identity(PortDirection::Input, descriptor.port_id.as_str()),
                value_contract: plot
                    .front
                    .value_contract(&FrontValueLocation::Input(descriptor.port_id.clone()))
                    .cloned(),
                descriptor,
            })
            .collect();
        graph.front_outputs = plot
            .front
            .outputs()
            .iter()
            .cloned()
            .map(|descriptor| PatchbayFrontPort {
                identity: front_port_identity(PortDirection::Output, descriptor.port_id.as_str()),
                value_contract: plot
                    .front
                    .value_contract(&FrontValueLocation::Output(descriptor.port_id.clone()))
                    .cloned(),
                descriptor,
            })
            .collect();
        let boundary_cords = plot.input_bindings.len() + plot.output_bindings.len();
        if graph
            .cords
            .len()
            .checked_add(boundary_cords)
            .is_none_or(|count| count > MAX_PATCHBAY_CORDS)
        {
            return Err(PatchbayGraphError::TooManyCords);
        }
        for binding in &plot.input_bindings {
            let source = front_port_identity(PortDirection::Input, binding.front_port_id.as_str());
            let sink = port_identity(
                &binding.gear_id,
                PortDirection::Input,
                binding.gear_port_id.as_str(),
            );
            let descriptor = graph
                .front_inputs
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
        for binding in &plot.output_bindings {
            let source = port_identity(
                &binding.gear_id,
                PortDirection::Output,
                binding.gear_port_id.as_str(),
            );
            let sink = front_port_identity(PortDirection::Output, binding.front_port_id.as_str());
            let descriptor = graph
                .front_outputs
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

fn patchbay_port(
    gear_id: &GearId,
    descriptor: &PortDescriptor,
    value_contract: Option<CheckedValueContract>,
) -> PatchbayPort {
    PatchbayPort {
        identity: port_identity(gear_id, descriptor.direction, descriptor.port_id.as_str()),
        gear_id: gear_id.clone(),
        descriptor: descriptor.clone(),
        value_contract,
    }
}

fn port_identity(gear: &GearId, direction: PortDirection, port: &str) -> String {
    let direction = match direction {
        PortDirection::Input => "input",
        PortDirection::Output => "output",
    };
    format!("port/{}/{direction}/{port}", gear.as_str())
}

fn front_port_identity(direction: PortDirection, port: &str) -> String {
    let direction = match direction {
        PortDirection::Input => "input",
        PortDirection::Output => "output",
    };
    format!("front/{direction}/{port}")
}
