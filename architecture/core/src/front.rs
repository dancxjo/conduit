use crate::{CapabilityOffer, KindId, PortDescriptor, PortId, ResourcePortContract};
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Exact location of a finite variable-size value in one callable Fore.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FrontValueLocation {
    Startup(String),
    Input(PortId),
    Output(PortId),
}

/// Semantic value envelope. This constrains info; it does not promise
/// allocation, retention, residence, or persistence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FrontValueBound {
    pub location: FrontValueLocation,
    pub maximum_bytes: u64,
}

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
    value_bounds: Vec<FrontValueBound>,
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
            value_bounds: Vec::new(),
        }
    }

    pub fn with_resource_ports(mut self, mut resource_ports: Vec<ResourcePortContract>) -> Self {
        resource_ports.sort_by(|left, right| left.port_id.cmp(&right.port_id));
        self.resource_ports = resource_ports;
        self
    }

    pub fn with_value_bounds(mut self, mut value_bounds: Vec<FrontValueBound>) -> Self {
        value_bounds.sort();
        self.value_bounds = value_bounds;
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

    pub fn value_bounds(&self) -> &[FrontValueBound] {
        &self.value_bounds
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
        .with_value_bounds(self.semantic_contract.value_bounds().to_vec())
    }
}
