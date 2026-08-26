use alloc::vec::Vec;
use conduit_core::{CapabilityLimits, KindContractRevision, KindId, PortDescriptor};

/// Host-neutral, finite execution meaning for one Kind revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableKindContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}
