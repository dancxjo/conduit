//! Fixed Root capability scope and gate claim types.
pub const MAXIMUM_DOMAIN_CAPABILITIES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectionDomainId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelCapabilityHandle(pub(super) u64);

impl KernelCapabilityHandle {
    #[cfg(any(
        feature = "conduitos-isolation-proof",
        all(target_arch = "x86", feature = "ia32-product"),
        all(target_os = "none", target_arch = "x86_64")
    ))]
    pub(crate) const fn raw_for_domain(self) -> u64 {
        self.0
    }

    #[cfg(any(
        feature = "conduitos-isolation-proof",
        all(target_arch = "x86", feature = "ia32-product"),
        all(target_os = "none", target_arch = "x86_64")
    ))]
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

impl KernelCapabilityRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidTable => "domain-capability-table-invalid",
            Self::InvalidScope => "domain-capability-scope-invalid",
            Self::TableFull => "domain-capability-table-full",
            Self::UnknownHandle => "domain-capability-unknown-handle",
            Self::WrongDomain => "domain-capability-wrong-domain",
            Self::WrongScope => "domain-capability-wrong-current-scope",
            Self::ParameterEnvelope => "domain-capability-parameter-envelope",
            Self::WorkEnvelope => "domain-capability-work-envelope",
            Self::InFlightFull => "domain-capability-in-flight-full",
            Self::Exhausted => "domain-capability-operation-exhausted",
            Self::Revoked => "domain-capability-revoked",
            Self::StaleLease => "domain-capability-stale-lease",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelRevocationCause {
    PlayCancelled,
    PlayCompleted,
    PlayFailed,
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
