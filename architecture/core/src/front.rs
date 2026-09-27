use crate::{CapabilityOffer, KindId, PortDescriptor, PortId, ResourcePortContract};
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FrontStartupParameter {
    pub name: String,
    /// Canonical semantic identity, never the source alias used to author it.
    pub value_type: KindId,
    pub has_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedFront {
    startup_parameters: Vec<FrontStartupParameter>,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    shorthand: Option<(PortId, PortId)>,
    #[serde(default)]
    resource_ports: Vec<ResourcePortContract>,
}

impl CheckedFront {
    pub fn new(
        startup_parameters: Vec<FrontStartupParameter>,
        mut inputs: Vec<PortDescriptor>,
        mut outputs: Vec<PortDescriptor>,
        shorthand: Option<(PortId, PortId)>,
    ) -> Self {
        inputs.sort_by(|left, right| left.port_id.as_str().cmp(right.port_id.as_str()));
        outputs.sort_by(|left, right| left.port_id.as_str().cmp(right.port_id.as_str()));
        Self {
            startup_parameters,
            inputs,
            outputs,
            shorthand,
            resource_ports: Vec::new(),
        }
    }

    pub fn with_resource_ports(mut self, mut resource_ports: Vec<ResourcePortContract>) -> Self {
        resource_ports.sort_by(|left, right| left.port_id.cmp(&right.port_id));
        self.resource_ports = resource_ports;
        self
    }

    pub fn startup_parameters(&self) -> &[FrontStartupParameter] {
        &self.startup_parameters
    }

    pub fn inputs(&self) -> &[PortDescriptor] {
        &self.inputs
    }

    pub fn outputs(&self) -> &[PortDescriptor] {
        &self.outputs
    }

    pub fn shorthand(&self) -> Option<(&PortId, &PortId)> {
        self.shorthand
            .as_ref()
            .map(|(input, output)| (input, output))
    }

    pub fn resource_ports(&self) -> &[ResourcePortContract] {
        &self.resource_ports
    }
}

impl CapabilityOffer {
    pub fn checked_front(&self) -> CheckedFront {
        let shorthand = self.shorthand.clone().or_else(|| {
            if let ([input], [output]) = (self.inputs.as_slice(), self.outputs.as_slice()) {
                Some((input.port_id.clone(), output.port_id.clone()))
            } else {
                None
            }
        });
        CheckedFront::new(
            self.startup_parameters.clone(),
            self.inputs.clone(),
            self.outputs.clone(),
            shorthand,
        )
    }
}
