//! Connection compatibility and finite composition admission.
use crate::{prelude::*, *};
use conduit_core::ConnectionTrack;
use conduit_plot::{validate_connection_contract, validate_front_contract};

impl PatchbayGraph {
    pub fn connection_candidates(&self, source_identity: &str) -> Vec<PatchbayConnectionCandidate> {
        self.gears
            .iter()
            .flat_map(|gear| &gear.inputs)
            .map(|port| port.identity.as_str())
            .chain(self.front_outputs.iter().map(|port| port.identity.as_str()))
            .chain(self.compositions.iter().flat_map(|composition| {
                composition.inputs.iter().map(|port| port.identity.as_str())
            }))
            .map(|sink_identity| PatchbayConnectionCandidate {
                sink_identity: sink_identity.to_owned(),
                compatibility: self.connection_compatibility(source_identity, sink_identity),
            })
            .collect()
    }

    pub fn connection_compatibility(
        &self,
        source_identity: &str,
        sink_identity: &str,
    ) -> PatchbayPortCompatibility {
        let source = self
            .gears
            .iter()
            .flat_map(|gear| &gear.outputs)
            .map(|port| (&port.identity, &port.descriptor))
            .chain(
                self.front_inputs
                    .iter()
                    .map(|port| (&port.identity, &port.descriptor)),
            )
            .chain(self.compositions.iter().flat_map(|composition| {
                composition
                    .outputs
                    .iter()
                    .map(|port| (&port.identity, &port.descriptor))
            }))
            .find_map(|(identity, descriptor)| (identity == source_identity).then_some(descriptor));
        let sink = self
            .gears
            .iter()
            .flat_map(|gear| &gear.inputs)
            .map(|port| (&port.identity, &port.descriptor))
            .chain(
                self.front_outputs
                    .iter()
                    .map(|port| (&port.identity, &port.descriptor)),
            )
            .chain(self.compositions.iter().flat_map(|composition| {
                composition
                    .inputs
                    .iter()
                    .map(|port| (&port.identity, &port.descriptor))
            }))
            .find_map(|(identity, descriptor)| (identity == sink_identity).then_some(descriptor));
        let (Some(source), Some(sink)) = (source, sink) else {
            let source_known = self
                .subject_identities()
                .any(|identity| identity == source_identity);
            let sink_known = self
                .subject_identities()
                .any(|identity| identity == sink_identity);
            return if source_known && sink_known {
                PatchbayPortCompatibility::InvalidDirection
            } else {
                PatchbayPortCompatibility::UnknownPort
            };
        };
        let source_front = self
            .front_inputs
            .iter()
            .any(|port| port.identity == source_identity);
        let sink_front = self
            .front_outputs
            .iter()
            .any(|port| port.identity == sink_identity);
        if source_front && sink_front {
            // Source expansion requires Fore passthrough to cross an admitted Gear.
            return PatchbayPortCompatibility::InvalidDirection;
        }
        let contract = if source_front {
            validate_front_contract(
                source.port_id.as_str(),
                &source.value_kind,
                source.temporal,
                sink,
                true,
            )
        } else if sink_front {
            validate_front_contract(
                sink.port_id.as_str(),
                &sink.value_kind,
                sink.temporal,
                source,
                false,
            )
        } else {
            validate_connection_contract(source, sink, ConnectionTrack::Payload)
        };
        if contract.is_err() {
            if source.value_kind != sink.value_kind {
                return PatchbayPortCompatibility::IncompatibleInfo {
                    source: source.value_kind.clone(),
                    sink: sink.value_kind.clone(),
                };
            }
            return PatchbayPortCompatibility::IncompatibleTemporal {
                source: source.temporal,
                sink: sink.temporal,
            };
        }
        let bound_source = self.compositions.iter().find_map(|composition| {
            composition
                .output_bindings
                .iter()
                .find(|binding| binding.front_port == source_identity)
                .map(|binding| binding.internal_port.as_str())
        });
        let bound_sink = self.compositions.iter().find_map(|composition| {
            composition
                .input_bindings
                .iter()
                .find(|binding| binding.front_port == sink_identity)
                .map(|binding| binding.internal_port.as_str())
        });
        if self.cords.iter().any(|cord| {
            cord.source_port == bound_source.unwrap_or(source_identity)
                && cord.sink_port == bound_sink.unwrap_or(sink_identity)
        }) {
            return PatchbayPortCompatibility::DuplicateCord;
        }
        PatchbayPortCompatibility::Compatible
    }

    pub fn subject_count(&self) -> usize {
        self.gears.len()
            + self.compositions.len()
            + self.front_inputs.len()
            + self.front_outputs.len()
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
        let existing_ports = self.front_inputs.len()
            + self.front_outputs.len()
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
}
