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
        let mut value_bounds = Vec::new();
        for parameter in &startup_parameters {
            if let Some(maximum_bytes) = default_maximum_bytes(&parameter.value_type) {
                value_bounds.push(FrontValueBound {
                    location: FrontValueLocation::Startup(parameter.name.clone()),
                    maximum_bytes,
                });
            }
        }
        for port in &inputs {
            if let Some(maximum_bytes) = default_maximum_bytes(&port.value_kind) {
                value_bounds.push(FrontValueBound {
                    location: FrontValueLocation::Input(port.port_id.clone()),
                    maximum_bytes,
                });
            }
        }
        for port in &outputs {
            if let Some(maximum_bytes) = default_maximum_bytes(&port.value_kind) {
                value_bounds.push(FrontValueBound {
                    location: FrontValueLocation::Output(port.port_id.clone()),
                    maximum_bytes,
                });
            }
        }
        value_bounds.sort();
        Self {
            startup_parameters,
            inputs,
            outputs,
            shorthand,
            resource_ports: Vec::new(),
            value_bounds,
        }
    }

    pub fn with_resource_ports(mut self, mut resource_ports: Vec<ResourcePortContract>) -> Self {
        resource_ports.sort_by(|left, right| left.port_id.cmp(&right.port_id));
        self.resource_ports = resource_ports;
        self
    }

    pub fn with_value_bounds(mut self, value_bounds: Vec<FrontValueBound>) -> Self {
        for bound in value_bounds {
            self.value_bounds
                .retain(|existing| existing.location != bound.location);
            self.value_bounds.push(bound);
        }
        self.value_bounds.sort();
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

fn default_maximum_bytes(kind_id: &KindId) -> Option<u64> {
    match kind_id.as_str() {
        "value/text" => Some(256),
        "value/bytes" => Some(65_536),
        _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn startup(name: &str, value_type: &str) -> FrontStartupParameter {
        FrontStartupParameter {
            name: name.into(),
            value_type: KindId::from(value_type),
            has_default: false,
        }
    }

    #[test]
    fn canonical_variable_size_kinds_supply_their_finite_default_envelopes() {
        let front = CheckedFront::new(
            vec![
                startup("message", "value/text"),
                startup("blob", "value/bytes"),
            ],
            Vec::new(),
            Vec::new(),
            None,
        );

        assert_eq!(
            front.value_bounds(),
            &[
                FrontValueBound {
                    location: FrontValueLocation::Startup("blob".into()),
                    maximum_bytes: 65_536,
                },
                FrontValueBound {
                    location: FrontValueLocation::Startup("message".into()),
                    maximum_bytes: 256,
                },
            ]
        );
    }

    #[test]
    fn explicit_value_bound_replaces_the_default_at_the_same_location() {
        let front = CheckedFront::new(
            vec![startup("message", "value/text")],
            Vec::new(),
            Vec::new(),
            None,
        )
        .with_value_bounds(vec![FrontValueBound {
            location: FrontValueLocation::Startup("message".into()),
            maximum_bytes: 32,
        }]);

        assert_eq!(front.value_bounds()[0].maximum_bytes, 32);
        assert_eq!(front.value_bounds().len(), 1);
    }
}
