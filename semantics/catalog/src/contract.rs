//! Portable kind contracts and their finite configuration/terminal behavior.
use alloc::{string::String, vec::Vec};
use conduit_core::{
    CapabilityLimits, FrontValueBound, FrontValueLocation, Kind, KindConfigurationField, KindId,
    KindSemanticLaw, KindTerminalBehavior, PortDescriptor, PortDirection,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardKindContract {
    pub kind_id: KindId,
    pub plain_name: String,
    pub summary: String,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<KindConfigurationField>,
    pub limits: CapabilityLimits,
    pub terminal_behavior: KindTerminalBehavior,
    pub hosted_implementation_required: bool,
    pub browser_manifestation_honest: bool,
    pub pico_manifestation_honest: bool,
    pub example: String,
}

impl StandardKindContract {
    pub fn into_semantic_contract(self, revision: &str) -> Kind {
        let value_bounds: Vec<FrontValueBound> = self
            .inputs
            .iter()
            .chain(&self.outputs)
            .filter_map(|port| {
                let maximum_bytes = match port.value_kind.as_str() {
                    "value/text" => 256,
                    "value/bytes" => 65_536,
                    _ => return None,
                };
                Some(FrontValueBound {
                    location: match port.direction {
                        PortDirection::Input => FrontValueLocation::Input(port.port_id.clone()),
                        PortDirection::Output => FrontValueLocation::Output(port.port_id.clone()),
                    },
                    maximum_bytes,
                })
            })
            .collect();
        let mut semantic_laws = alloc::vec![KindSemanticLaw::Terminal(self.terminal_behavior)];
        if !value_bounds.is_empty() {
            semantic_laws.push(KindSemanticLaw::ValueBounds(value_bounds));
        }
        Kind {
            startup_parameters: crate::startup_front(&self.configuration),
            shorthand: None,
            kind_id: self.kind_id,
            kind_contract_revision: revision.into(),
            inputs: self.inputs,
            outputs: self.outputs,
            configuration: self.configuration,
            semantic_laws,
            limits: self.limits,
        }
    }
}
