//! Stable Patchbay boundaries for recursively realized Form Gears.

use crate::{
    PatchbayComposition, PatchbayCompositionBinding, PatchbayFacePort, PatchbayGraph,
    PatchbayGraphError, RecursiveFormGearProjection,
};
use conduit_core::{GearId, PortDescriptor};

impl PatchbayGraph {
    /// Adds one presentation boundary for realization truth already projected
    /// from an admitted recursive Form Back. The flattened Gears and Cords
    /// remain authoritative; this records only how Patchbay may collapse that
    /// graph behind the checked Face.
    pub fn admit_recursive_form(
        &mut self,
        projection: &RecursiveFormGearProjection,
    ) -> Result<(), PatchbayGraphError> {
        if projection.expanded_form_id != self.expanded_form_id {
            return Err(PatchbayGraphError::StaleGraphBasis);
        }
        let identity = format!("composition/{}", projection.invocation_path);
        let port_identity = |direction: &str, port: &PortDescriptor| {
            format!("{identity}/{direction}/{}", port.port_id.as_str())
        };
        let internal_port = |gear: &GearId, direction: &str, port: &str| {
            format!("port/{}/{direction}/{port}", gear.as_str())
        };
        let inputs = projection
            .face
            .inputs()
            .iter()
            .cloned()
            .map(|descriptor| PatchbayFacePort {
                identity: port_identity("input", &descriptor),
                descriptor,
            })
            .collect::<Vec<_>>();
        let outputs = projection
            .face
            .outputs()
            .iter()
            .cloned()
            .map(|descriptor| PatchbayFacePort {
                identity: port_identity("output", &descriptor),
                descriptor,
            })
            .collect::<Vec<_>>();
        let mut input_bindings = Vec::new();
        let mut output_bindings = Vec::new();
        for connection in &projection.boundary_connections {
            let source_nested = connection
                .source_gear_id
                .as_str()
                .starts_with(&format!("{}/", projection.invocation_path));
            let ports = if source_nested { &outputs } else { &inputs };
            let mut compatible = ports.iter().filter(|port| {
                port.descriptor.value_kind == connection.value_kind
                    && port.descriptor.temporal == connection.temporal
            });
            let Some(face) = compatible.next() else {
                return Err(PatchbayGraphError::MissingCordEndpoint);
            };
            if compatible.next().is_some() {
                return Err(PatchbayGraphError::CordContractMismatch);
            }
            let (bindings, gear, direction, port) = if source_nested {
                (
                    &mut output_bindings,
                    &connection.source_gear_id,
                    "output",
                    connection.source_port_id.as_str(),
                )
            } else {
                (
                    &mut input_bindings,
                    &connection.sink_gear_id,
                    "input",
                    connection.sink_port_id.as_str(),
                )
            };
            bindings.push(PatchbayCompositionBinding {
                face_port: face.identity.clone(),
                internal_port: internal_port(gear, direction, port),
            });
        }
        self.admit_composition(PatchbayComposition {
            identity,
            gear_name: projection.invocation_path.clone(),
            back_name: projection.source_document_id.as_str().into(),
            checked_form_id: projection.checked_form_id.clone(),
            inputs,
            outputs,
            input_bindings,
            output_bindings,
        })
    }
}
