//! Portable kind contracts and their finite configuration/terminal behavior.
use alloc::{string::String, vec::Vec};
use conduit_core::{
    CapabilityLimits, Kind, KindConfigurationField, KindId, KindSemanticLaw, KindTerminalBehavior,
    PortDescriptor,
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
        Kind {
            startup_parameters: crate::startup_front(&self.configuration),
            shorthand: None,
            kind_id: self.kind_id,
            kind_contract_revision: revision.into(),
            inputs: self.inputs,
            outputs: self.outputs,
            configuration: self.configuration,
            semantic_laws: alloc::vec![KindSemanticLaw::Terminal(self.terminal_behavior)],
            limits: self.limits,
        }
    }
}
