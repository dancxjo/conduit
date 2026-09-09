//! Connection compatibility and finite composition admission.
use crate::{prelude::*, *};
use conduit_core::PortTemporal;

impl PatchbayGraph {
    pub fn connection_candidates(&self, source_identity: &str) -> Vec<PatchbayConnectionCandidate> {
        self.gears
            .iter()
            .flat_map(|gear| &gear.inputs)
            .map(|port| port.identity.as_str())
            .chain(self.face_outputs.iter().map(|port| port.identity.as_str()))
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
                self.face_inputs
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
                self.face_outputs
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
        if source.value_kind != sink.value_kind {
            return PatchbayPortCompatibility::IncompatibleInfo {
                source: source.value_kind.clone(),
                sink: sink.value_kind.clone(),
            };
        }
        if !temporal_compatible(source.temporal, sink.temporal) {
            return PatchbayPortCompatibility::IncompatibleTemporal {
                source: source.temporal,
                sink: sink.temporal,
            };
        }
        let bound_source = self.compositions.iter().find_map(|composition| {
            composition
                .output_bindings
                .iter()
                .find(|binding| binding.face_port == source_identity)
                .map(|binding| binding.internal_port.as_str())
        });
        let bound_sink = self.compositions.iter().find_map(|composition| {
            composition
                .input_bindings
                .iter()
                .find(|binding| binding.face_port == sink_identity)
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
}

pub(crate) fn temporal_compatible(source: PortTemporal, sink: PortTemporal) -> bool {
    source == sink
        || matches!(
            (source, sink),
            (PortTemporal::Flow { .. }, PortTemporal::Value)
                | (
                    PortTemporal::Flow { closes: true },
                    PortTemporal::Flow { closes: false }
                )
        )
}
