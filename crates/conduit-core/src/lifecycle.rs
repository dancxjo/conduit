use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use super::*;

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
    InvalidStartupDependencies,
    UnsupportedCancellationPolicy,
    UnsupportedTerminalPolicy,
    EvidenceBudgetExceeded,
    HostOperationContractMismatch,
    HostOperationNotPlanned,
    HostOperationInputExceeded,
    HostOperationOutputExceeded,
    ResourceContractMismatch,
    ResourceCapacityExceeded,
    AuthorityContractMismatch,
    AuthorityDenied,
    LinkBindingMismatch,
    LinkUnavailable,
    ConnectionDisconnected,
    MalformedConnectionEnvelope,
    StalePlan,
    CompositeCapabilityFailed,
    UnknownImplementation,
    UnsupportedKind,
    ImplementationKindMismatch,
    KindContractRevisionMismatch,
    ExecutionProfileMismatch,
    PortContractMismatch,
    AdvertisedImplementationMismatch,
    ArtifactIdentityMismatch,
    PlanIdentityMismatch,
    InvalidOperationConfiguration,
    UnsupportedValueKind,
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
    pub last_manifested_sequence: Option<u64>,
    pub undeliverable_items: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub evidence_id: EvidenceId,
    pub active_play_id: Option<ActivePlayId>,
    pub presentation_id: Option<PresentationId>,
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
        reason: FailureReason,
        message: Option<String>,
    },
    Cancelled,
    Released,
    EvidenceGap {
        dropped: u64,
    },
}

// The allocator-free host boundary keeps the sealed preparation fragment inline. Boxing the
// largest variant would make every no-std host provide allocation for command admission.
#[allow(clippy::large_enum_variant)]
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
        active_play_id: ActivePlayId,
        presentation_id: PresentationId,
        placement_id: PlacementId,
        value: ValuePayload,
        success: bool,
        message: Option<String>,
    },
    AcceptConnectionEnvelope(ConnectionEnvelope),
    CompleteConnectionDelivery {
        plan_id: PlanId,
        connection_id: ConnectionId,
        sequence: u64,
        outcome: ConnectionOutcome,
    },
    CloseConnection {
        plan_id: PlanId,
        connection_id: ConnectionId,
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
        reason: FailureReason,
        message: Option<String>,
    },
    Activated {
        plan_id: PlanId,
        active_play_id: ActivePlayId,
    },
    ActivationRejected {
        plan_id: PlanId,
        reason: FailureReason,
        message: Option<String>,
    },
    TimerRequested {
        plan_id: PlanId,
        placement_id: PlacementId,
        duration_ms: u64,
    },
    PresentValueRequested {
        plan_id: PlanId,
        active_play_id: ActivePlayId,
        presentation_id: PresentationId,
        placement_id: PlacementId,
        presentation_kind: KindId,
        value: ValuePayload,
    },
    ConnectionBlocked {
        plan_id: PlanId,
        connection_id: ConnectionId,
    },
    ConnectionEnvelopeOutcome {
        plan_id: PlanId,
        connection_id: ConnectionId,
        sequence: u64,
        outcome: ConnectionOutcome,
    },
    ValueDelivered {
        plan_id: PlanId,
        connection_id: ConnectionId,
        value: ValuePayload,
    },
    ManifestationCompleted {
        plan_id: PlanId,
        active_play_id: ActivePlayId,
        presentation_id: PresentationId,
        placement_id: PlacementId,
        value: ValuePayload,
    },
    ManifestationFailed {
        plan_id: PlanId,
        active_play_id: ActivePlayId,
        presentation_id: PresentationId,
        placement_id: PlacementId,
        value: ValuePayload,
        reason: FailureReason,
        message: Option<String>,
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
    MandatoryEvidenceReports {
        items: Vec<MandatoryEvidenceReport>,
    },
}

/// Host-neutral work requested by an installed semantic implementation.
///
/// A std adapter may map `PresentValue` to stdout, a browser adapter may map it to DOM
/// presentation, and a Pico W adapter may map it to an LED. Those manifestations are adapter
/// policy; the semantic operation remains unaware of the platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformEffect {
    Wait {
        plan_id: PlanId,
        placement_id: PlacementId,
        duration_ms: u64,
    },
    PresentValue {
        plan_id: PlanId,
        active_play_id: ActivePlayId,
        presentation_id: PresentationId,
        placement_id: PlacementId,
        presentation_kind: KindId,
        value: ValuePayload,
    },
    TransmitConnection {
        envelope: ConnectionEnvelope,
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

    pub fn front(&self) -> Option<&T> {
        self.items.front()
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

pub fn wait_host_operation_requirement() -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(WAIT_HOST_OPERATION_CONTRACT),
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: core::mem::size_of::<u64>() as u32,
        maximum_output_bytes: 0,
    }
}

pub fn present_host_operation_requirement(
    target_kind: KindId,
    maximum_input_bytes: u32,
) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(PRESENT_HOST_OPERATION_CONTRACT),
        target_kind: Some(target_kind),
        maximum_in_flight: 1,
        maximum_input_bytes,
        maximum_output_bytes: MAX_PRESENTATION_COMPLETION_BYTES,
    }
}

pub fn resource_requirement(class_id: &str, units: u32) -> ResourceRequirement {
    ResourceRequirement {
        class_id: ResourceClassId::from(class_id),
        units,
    }
}

pub fn resource_offer(pool_id: &str, class_id: &str, capacity_units: u32) -> ResourceOffer {
    ResourceOffer {
        pool_id: ResourcePoolId::from(pool_id),
        class_id: ResourceClassId::from(class_id),
        capacity_units,
    }
}

pub fn present_authority_requirement(subject_kind: KindId) -> AuthorityRequirement {
    AuthorityRequirement {
        contract_id: AuthorityContractId::from(PRESENT_AUTHORITY_CONTRACT),
        host_operation_contract_id: HostOperationContractId::from(PRESENT_HOST_OPERATION_CONTRACT),
        subject_kind,
    }
}

pub fn authority_grant(
    grant_id: &str,
    requirement: &AuthorityRequirement,
    host_id: HostId,
    boot_id: BootId,
    capability_id: CapabilityId,
) -> AuthorityGrant {
    AuthorityGrant {
        grant_id: AuthorityGrantId::from(grant_id),
        contract_id: requirement.contract_id.clone(),
        host_operation_contract_id: requirement.host_operation_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id,
        boot_id,
        capability_id,
    }
}

/// Build a ready link observation for a provider whose endpoint access is
/// wholly owned by the current process. This is suitable for deterministic
/// in-process fixtures; actual carriers should supply explicit credential and
/// grant references instead.
pub fn process_owned_link_binding(
    binding_id: &str,
    provider: ConnectionProvider,
    provider_instance_id: &str,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    maximum_in_flight_items: u16,
    maximum_buffered_bytes: u32,
) -> LinkBinding {
    process_owned_link_binding_with_limits(
        binding_id,
        provider,
        provider_instance_id,
        source,
        sink,
        LinkLimits {
            maximum_in_flight_items,
            maximum_payload_bytes: maximum_buffered_bytes,
            maximum_buffered_bytes,
            maximum_frame_bytes: maximum_buffered_bytes,
        },
    )
}

pub fn process_owned_link_binding_with_limits(
    binding_id: &str,
    provider: ConnectionProvider,
    provider_instance_id: &str,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    limits: LinkLimits,
) -> LinkBinding {
    LinkBinding {
        binding_id: LinkBindingId::from(binding_id),
        source: LinkEndpoint {
            host_id: source.host_id.clone(),
            boot_id: source.boot_id.clone(),
            endpoint_id: LinkEndpointId::from(format!("{binding_id}/source")),
        },
        sink: LinkEndpoint {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
            endpoint_id: LinkEndpointId::from(format!("{binding_id}/sink")),
        },
        provider,
        provider_instance_id: ConnectionProviderInstanceId::from(provider_instance_id),
        availability: LinkAvailability::Ready,
        credential: LinkCredentialReference::None,
        authority: LinkAuthorityReference::ProcessOwned,
        limits,
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{
        bind_active_play, bind_evidence, bind_presentation, mandatory_evidence_storage_requirement,
        BootId, EvidenceStorageBudget, ExpectedEvidence, HostId, PlacementId, PlanId,
    };

    #[test]
    fn mandatory_evidence_budget_counts_items_and_identity_bytes_independently() {
        let evidence = vec![
            ExpectedEvidence::PlanFragmentReceived,
            ExpectedEvidence::PlacementPrepared(PlacementId::from("abc")),
            ExpectedEvidence::PlanTerminal,
        ];
        assert_eq!(
            mandatory_evidence_storage_requirement(&evidence),
            Some(EvidenceStorageBudget {
                item_capacity: 3,
                byte_capacity: 6,
            })
        );
    }

    #[test]
    fn execution_identity_chain_keeps_plan_play_evidence_and_presentation_distinct() {
        let plan_id = PlanId::from("plan/exact");
        let host_id = HostId::from("host/exact");
        let boot_id = BootId::from("boot/exact");
        let active = bind_active_play(&plan_id, &host_id, &boot_id, 7);
        let evidence = bind_evidence(&host_id, &boot_id, Some(&active.active_play_id), 11);
        let presentation = bind_presentation(
            &active.active_play_id,
            &PlacementId::from("placement/show"),
            3,
        );

        assert_eq!(active.plan_id, plan_id);
        assert_eq!(evidence.active_play_id, Some(active.active_play_id.clone()));
        assert_eq!(presentation.active_play_id, active.active_play_id);
        assert_ne!(active.active_play_id.as_str(), plan_id.as_str());
        assert_ne!(evidence.evidence_id.as_str(), plan_id.as_str());
        assert_ne!(presentation.presentation_id.as_str(), plan_id.as_str());
        assert_ne!(
            evidence.evidence_id.as_str(),
            presentation.presentation_id.as_str()
        );
        assert_ne!(
            bind_active_play(&plan_id, &host_id, &boot_id, 8).active_play_id,
            active.active_play_id
        );
        assert_ne!(
            bind_active_play(&plan_id, &host_id, &BootId::from("boot/restarted"), 7).active_play_id,
            active.active_play_id
        );
    }
}
