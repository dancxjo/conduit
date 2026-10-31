//! Descriptive identity copied from independently retained native equipment.
//! These values confer no execution authority or Body membership.
use super::UsbSourceRoot;
use conduit_core::{BaseInstanceId, BootId, CapabilityId, HostId};

/// Local association between checked instrument context and its retained Source.
/// Only the native owner constructs this description; copying it does not copy
/// possession, issue a capability, or establish continuity across reacquisition.
#[derive(Clone)]
pub struct SourceSessionDescriptor {
    host: HostId,
    boot: BootId,
    capability: CapabilityId,
    session: BaseInstanceId,
}

impl SourceSessionDescriptor {
    pub fn host(&self) -> &HostId {
        &self.host
    }
    pub fn boot(&self) -> &BootId {
        &self.boot
    }
    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }
    pub fn session(&self) -> &BaseInstanceId {
        &self.session
    }
}

impl<P> UsbSourceRoot<P> {
    pub fn source_descriptor(&self) -> SourceSessionDescriptor {
        SourceSessionDescriptor {
            host: self.identity.host_id.clone(),
            boot: self.identity.boot_id.clone(),
            capability: self.identity.capability.clone(),
            session: self.identity.provider_instance_id.clone(),
        }
    }
}
