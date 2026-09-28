use crate::{
    CapabilityOffer, CheckedValueContract, KindId, PortDescriptor, PortId, ResourcePortContract,
};
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
pub struct FrontValueContract {
    pub location: FrontValueLocation,
    pub contract: CheckedValueContract,
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
    value_contracts: Vec<FrontValueContract>,
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
        let mut value_contracts = Vec::new();
        for parameter in &startup_parameters {
            if let Some(maximum_bytes) = default_maximum_bytes(&parameter.value_type) {
                value_contracts.push(FrontValueContract {
                    location: FrontValueLocation::Startup(parameter.name.clone()),
                    contract: CheckedValueContract::new(
                        parameter.value_type.clone(),
                        maximum_bytes as u32,
                        Vec::new(),
                    )
                    .expect("canonical default value contract is finite"),
                });
            }
        }
        for port in &inputs {
            if let Some(maximum_bytes) = default_maximum_bytes(&port.value_kind) {
                value_contracts.push(FrontValueContract {
                    location: FrontValueLocation::Input(port.port_id.clone()),
                    contract: CheckedValueContract::new(
                        port.value_kind.clone(),
                        maximum_bytes as u32,
                        Vec::new(),
                    )
                    .expect("canonical default value contract is finite"),
                });
            }
        }
        for port in &outputs {
            if let Some(maximum_bytes) = default_maximum_bytes(&port.value_kind) {
                value_contracts.push(FrontValueContract {
                    location: FrontValueLocation::Output(port.port_id.clone()),
                    contract: CheckedValueContract::new(
                        port.value_kind.clone(),
                        maximum_bytes as u32,
                        Vec::new(),
                    )
                    .expect("canonical default value contract is finite"),
                });
            }
        }
        value_contracts.sort();
        Self {
            startup_parameters,
            inputs,
            outputs,
            shorthand,
            resource_ports: Vec::new(),
            value_contracts,
        }
    }

    pub fn with_resource_ports(mut self, mut resource_ports: Vec<ResourcePortContract>) -> Self {
        resource_ports.sort_by(|left, right| left.port_id.cmp(&right.port_id));
        self.resource_ports = resource_ports;
        self
    }

    pub fn with_value_contracts(mut self, value_contracts: Vec<FrontValueContract>) -> Self {
        for contract in value_contracts {
            self.value_contracts
                .retain(|existing| existing.location != contract.location);
            self.value_contracts.push(contract);
        }
        self.value_contracts.sort();
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

    pub fn value_contracts(&self) -> &[FrontValueContract] {
        &self.value_contracts
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
        .with_value_contracts(self.semantic_contract.value_contracts().to_vec())
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
            front.value_contracts(),
            &[
                FrontValueContract {
                    location: FrontValueLocation::Startup("blob".into()),
                    contract: CheckedValueContract::new(
                        KindId::from("value/bytes"),
                        65_536,
                        vec![],
                    )
                    .unwrap(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Startup("message".into()),
                    contract: CheckedValueContract::new(KindId::from("value/text"), 256, vec![],)
                        .unwrap(),
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
        .with_value_contracts(vec![FrontValueContract {
            location: FrontValueLocation::Startup("message".into()),
            contract: CheckedValueContract::new(KindId::from("value/text"), 32, vec![]).unwrap(),
        }]);

        assert_eq!(front.value_contracts()[0].contract.maximum_bytes, 32);
        assert_eq!(front.value_contracts().len(), 1);
    }

    #[test]
    fn constraints_participate_in_exact_checked_fore_identity() {
        let unconstrained = CheckedFront::new(
            vec![startup("message", "value/text")],
            Vec::new(),
            Vec::new(),
            None,
        );
        let constrained = unconstrained
            .clone()
            .with_value_contracts(vec![FrontValueContract {
                location: FrontValueLocation::Startup("message".into()),
                contract: CheckedValueContract::new(
                    KindId::from("value/text"),
                    256,
                    vec![crate::ValueConstraint::ByteLength {
                        minimum: 1,
                        maximum: 32,
                    }],
                )
                .unwrap(),
            }]);

        assert_ne!(
            crate::compute_checked_front_fingerprint(&unconstrained),
            crate::compute_checked_front_fingerprint(&constrained)
        );
    }
}
