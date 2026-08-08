use crate::{CheckedCanonicalForm, CheckedOperation, RuntimePort, RuntimePortDirection};

impl CheckedOperation {
    pub fn checked_face(&self) -> conduit_core::CheckedFace {
        conduit_core::CheckedFace::new(
            self.startup_parameters.clone(),
            self.inputs.clone(),
            self.outputs.clone(),
            self.shorthand.clone(),
        )
    }
}

impl CheckedCanonicalForm {
    pub fn checked_face(&self) -> conduit_core::CheckedFace {
        let inputs = self
            .runtime_ports
            .iter()
            .filter(|port| port.direction == RuntimePortDirection::Input)
            .map(face_port)
            .collect();
        let outputs = self
            .runtime_ports
            .iter()
            .filter(|port| port.direction == RuntimePortDirection::Output)
            .map(face_port)
            .collect();
        conduit_core::CheckedFace::new(
            self.startup_parameters
                .iter()
                .map(|parameter| conduit_core::FaceStartupParameter {
                    name: parameter.name.clone(),
                    value_type: parameter.value_type.clone(),
                    has_default: parameter.default.is_some(),
                })
                .collect(),
            inputs,
            outputs,
            self.shorthand.as_ref().map(|(input, output)| {
                (conduit_core::port_id(input), conduit_core::port_id(output))
            }),
        )
    }
}

fn face_port(port: &RuntimePort) -> conduit_core::PortDescriptor {
    conduit_core::PortDescriptor {
        port_id: conduit_core::port_id(&port.name.text),
        value_kind: crate::value_type::canonical_value_kind(&port.value_type.text),
        direction: match port.direction {
            RuntimePortDirection::Input => conduit_core::PortDirection::Input,
            RuntimePortDirection::Output => conduit_core::PortDirection::Output,
        },
        temporal: crate::value_type::canonical_port_temporal(port.temporal),
    }
}
