use alloc::vec::Vec;
use conduit_core::{
    CapabilityLimits, KindContractRevision, KindId, PortDescriptor, SemanticCapabilityContract,
};

/// Host-neutral, finite execution meaning for one kind revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableKindContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

impl PortableKindContract {
    pub fn into_semantic_contract(self) -> SemanticCapabilityContract {
        SemanticCapabilityContract {
            startup_parameters: Vec::new(),
            shorthand: None,
            kind_id: self.kind_id,
            kind_contract_revision: self.kind_contract_revision,
            inputs: self.inputs,
            outputs: self.outputs,
            limits: self.limits,
        }
    }
}
