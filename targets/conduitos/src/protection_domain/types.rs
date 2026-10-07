//! Fixed Root capability scope and gate claim types.
pub const MAXIMUM_DOMAIN_CAPABILITIES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectionDomainId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelCapabilityHandle(pub(super) u64);

impl KernelCapabilityHandle {
    #[cfg(feature = "conduitos-isolation-proof")]
    pub(crate) const fn raw_for_domain(self) -> u64 {
        self.0
    }

    #[cfg(feature = "conduitos-isolation-proof")]
    pub(crate) const fn from_untrusted(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelCapabilityScope {
    pub host: [u8; 32],
    pub boot: [u8; 32],
    pub plan: [u8; 32],
    pub play: [u8; 32],
    pub implementation: [u8; 32],
    pub base: [u8; 32],
    pub base_generation: u32,
    pub resource: [u8; 32],
    pub resource_generation: u32,
    pub operation: u32,
    pub subject: [u8; 32],
    pub authority: [u8; 32],
    pub maximum_parameter_bytes: u32,
    pub maximum_work_units: u32,
    pub maximum_in_flight: u16,
    pub maximum_operations: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelOperationClaim {
    pub boot: [u8; 32],
    pub plan: [u8; 32],
    pub play: [u8; 32],
    pub base_generation: u32,
    pub resource_generation: u32,
    pub operation: u32,
    pub parameter_bytes: u32,
    pub work_units: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelOperationLease {
    pub(super) slot: u16,
    pub(super) table_generation: u32,
    pub(super) sequence: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelCapabilityRefusal {
    InvalidTable,
    InvalidScope,
    TableFull,
    UnknownHandle,
    WrongDomain,
    WrongScope,
    ParameterEnvelope,
    WorkEnvelope,
    InFlightFull,
    Exhausted,
    Revoked,
    StaleLease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelRevocationCause {
    PlayCancelled,
    PlayCompleted,
    PlanReplaced,
    AuthorityRevoked,
    ResourceReplaced,
    BaseReplaced,
    BootReplaced,
    ProtectionFault,
    ProviderLost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelRevocationReceipt {
    pub cause: KernelRevocationCause,
    pub revoked_handles: u16,
}
