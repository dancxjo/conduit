#![no_std]

extern crate alloc;

use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const DEFAULT_CONNECTION_ITEM_CAPACITY: u16 = 4;
pub const DEFAULT_CONNECTION_BYTE_CAPACITY: u32 = 64;

macro_rules! identity_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

identity_type!(HostId);
identity_type!(BootId);
identity_type!(CapabilityId);
identity_type!(KindId);
identity_type!(ImplementationId);
identity_type!(FormId);
identity_type!(PlanId);
identity_type!(PlacementId);
identity_type!(ConnectionId);
identity_type!(PortId);
identity_type!(OperationId);
identity_type!(HostProfileId);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OfferGeneration(pub u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortDescriptor {
    pub port_id: PortId,
    pub value_kind: KindId,
    pub direction: PortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigurationValue {
    Bool(bool),
    U64(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationEntry {
    pub key: String,
    pub value: ConfigurationValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValuePayload {
    pub value_kind: KindId,
    pub encoded: Vec<u8>,
}

impl ValuePayload {
    pub fn encoded_len(&self) -> u32 {
        self.encoded.len() as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityLimits {
    pub value_kind: KindId,
    pub max_active_instances: u16,
    pub max_queue_items: u16,
    pub max_queue_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityOffer {
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub implementation_id: ImplementationId,
    pub limits: CapabilityLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAdvertisement {
    pub protocol_version: u16,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub profile: HostProfileId,
    pub capabilities: Vec<CapabilityOffer>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionProvider {
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedOperation {
    pub placement_id: PlacementId,
    pub operation_id: OperationId,
    pub kind_id: KindId,
    pub configuration: Vec<ConfigurationEntry>,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedConnection {
    pub connection_id: ConnectionId,
    pub source_placement_id: PlacementId,
    pub source_port_id: PortId,
    pub sink_placement_id: PlacementId,
    pub sink_port_id: PortId,
    pub value_kind: KindId,
    pub provider: ConnectionProvider,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanFragment {
    pub plan_id: PlanId,
    pub form_id: FormId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub placements: Vec<PlannedOperation>,
    pub connections: Vec<PlannedConnection>,
    pub startup_order: Vec<PlacementId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub plan_id: PlanId,
    pub form_id: FormId,
    pub fragments: Vec<PlanFragment>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementLifecycleState {
    Proposed,
    Prepared,
    Active,
    Completed,
    Failed,
    Cancelled,
    Released,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureReason {
    WrongHostIdentity,
    StaleBootIdentity,
    StaleOfferGeneration,
    UnknownCapability,
    CapabilityInstanceLimitExceeded,
    QueueCapacityExceeded,
    ByteCapacityExceeded,
    ManifestationFailed,
    RequiredBranchFailed,
    InvalidLifecycleCommand,
    LatePlatformCompletion,
    EvidenceGap,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancellationReason {
    OperatorRequested,
    RequiredPlanFailed,
    Released,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalDisposition {
    Completed,
    Failed { reason: FailureReason },
    Cancelled { reason: CancellationReason },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionTerminalDisposition {
    pub disposition: TerminalDisposition,
    pub last_accepted_sequence: Option<u64>,
    pub undeliverable_items: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub plan_id: Option<PlanId>,
    pub placement_id: Option<PlacementId>,
    pub connection_id: Option<ConnectionId>,
    pub kind: ObservationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationKind {
    HostStarted,
    AdvertisementPublished,
    PlanFragmentReceived,
    PlacementPrepared,
    PlanActivated,
    ValueProduced {
        value: ValuePayload,
    },
    ValueAccepted {
        value: ValuePayload,
    },
    ValuePresented {
        value: ValuePayload,
    },
    PlacementCompleted,
    PlanCompleted,
    PlacementTerminal {
        disposition: TerminalDisposition,
    },
    ConnectionTerminal {
        disposition: ConnectionTerminalDisposition,
    },
    PlanTerminal {
        disposition: TerminalDisposition,
    },
    Failure {
        reason: String,
    },
    Cancelled,
    Released,
    EvidenceGap {
        dropped: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostCommand {
    PublishAdvertisement(HostAdvertisement),
    Prepare(PlanFragment),
    Activate(PlanId),
    CompleteWait {
        plan_id: PlanId,
        placement_id: PlacementId,
    },
    CompletePresentation {
        plan_id: PlanId,
        placement_id: PlacementId,
        value: ValuePayload,
        success: bool,
        message: Option<String>,
    },
    Cancel(PlanId),
    Release(PlanId),
    Inspect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostEvent {
    Prepared {
        plan_id: PlanId,
    },
    PreparationRejected {
        plan_id: PlanId,
        reason: String,
    },
    Activated {
        plan_id: PlanId,
    },
    ActivationRejected {
        plan_id: PlanId,
        reason: String,
    },
    TimerRequested {
        plan_id: PlanId,
        placement_id: PlacementId,
        duration_ms: u64,
    },
    PresentValueRequested {
        plan_id: PlanId,
        placement_id: PlacementId,
        value: ValuePayload,
    },
    ConnectionBlocked {
        plan_id: PlanId,
        connection_id: ConnectionId,
    },
    ValueDelivered {
        plan_id: PlanId,
        connection_id: ConnectionId,
        value: ValuePayload,
    },
    ManifestationCompleted {
        plan_id: PlanId,
        placement_id: PlacementId,
        value: ValuePayload,
    },
    ManifestationFailed {
        plan_id: PlanId,
        placement_id: PlacementId,
        value: ValuePayload,
        reason: String,
    },
    PlacementCompleted {
        plan_id: PlanId,
        placement_id: PlacementId,
    },
    PlanCompleted {
        plan_id: PlanId,
    },
    PlacementTerminated {
        plan_id: PlanId,
        placement_id: PlacementId,
        disposition: TerminalDisposition,
    },
    ConnectionTerminated {
        plan_id: PlanId,
        connection_id: ConnectionId,
        disposition: ConnectionTerminalDisposition,
    },
    PlanTerminated {
        plan_id: PlanId,
        disposition: TerminalDisposition,
    },
    Cancelled {
        plan_id: PlanId,
    },
    Released {
        plan_id: PlanId,
    },
    CommandRejected {
        plan_id: Option<PlanId>,
        reason: FailureReason,
    },
    Observations {
        items: Vec<Observation>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformEffect {
    Wait {
        plan_id: PlanId,
        placement_id: PlacementId,
        duration_ms: u64,
    },
    PresentValue {
        plan_id: PlanId,
        placement_id: PlacementId,
        value: ValuePayload,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedQueue<T> {
    capacity: usize,
    items: VecDeque<T>,
}

impl<T> BoundedQueue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            items: VecDeque::new(),
        }
    }

    pub fn push(&mut self, item: T) -> Result<(), T> {
        if self.items.len() >= self.capacity {
            return Err(item);
        }
        self.items.push_back(item);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

pub fn kind_id(value: &str) -> KindId {
    KindId::from(value)
}

pub fn port_id(value: &str) -> PortId {
    PortId::from(value)
}
