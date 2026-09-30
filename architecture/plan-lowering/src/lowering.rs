//! Exact pre-play-start lowering from string-identified plan facts into the
//! numeric tables consumed by `conduit-kernel`.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use conduit_core::{
    ActivePlayId, ActivePlayIdentity, AdmittedLine, BootId, ConnectionId, ConnectionTrack,
    ExpectedSign, FragmentId, HostCallContractId, HostId, KindId, LinkEndpoint, PlacementId,
    PlanFragment, PlanId, PortDirection, PortId as PlanPortId, PortTemporal, PresentationId,
    PresentationIdentity, ResourceBinding as PlanResourceBinding, SharedPoolId, SignId,
    SignIdentity,
};
use conduit_kernel::{
    scheduler::{AssignedPressurePolicy, CordCapacity, CordSpec, NodeSpec},
    CordId, HostCallBinding, HostCallId, NodeId, PortId, RemoteEndpointId,
    ResourceBinding as KernelResourceBinding, ResourceId, RouteRange, RouteTarget,
    SignExpectationId, SignExpectationTarget,
};

mod admission;
mod fusion;
mod ports;
mod profile;
mod state;
use ports::{find_port, lower_ports};
pub use state::LoweredState;
mod remote;
mod shared_pool;
use fusion::lower_fusions;
pub use fusion::LoweredFusion;
pub use profile::{
    KernelStorageProfile, KernelStorageProfileError, FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
    FIXED_KERNEL_STORAGE_PROFILE,
};
use remote::lower_remote_endpoints;
use shared_pool::lower_shared_pools;
pub use shared_pool::{
    LoweredPoolRealization, LoweredPoolSelectionFacts, LoweredSharedPool,
    PoolObservationLoweringError,
};

fn lower_pressure_policy(policy: conduit_core::DeliveryPressurePolicy) -> AssignedPressurePolicy {
    match policy {
        conduit_core::DeliveryPressurePolicy::PreserveOrder => {
            AssignedPressurePolicy::PreserveOrder
        }
        conduit_core::DeliveryPressurePolicy::CoalesceLatest => {
            AssignedPressurePolicy::CoalesceLatest
        }
    }
}

fn lower_connection_track(
    track: conduit_core::ConnectionTrack,
) -> conduit_kernel::scheduler::AssignedConnectionTrack {
    match track {
        conduit_core::ConnectionTrack::Payload => {
            conduit_kernel::scheduler::AssignedConnectionTrack::Payload
        }
        conduit_core::ConnectionTrack::NormalClose => {
            conduit_kernel::scheduler::AssignedConnectionTrack::NormalClose
        }
        conduit_core::ConnectionTrack::AbnormalTerminal => {
            conduit_kernel::scheduler::AssignedConnectionTrack::AbnormalTerminal
        }
        conduit_core::ConnectionTrack::Quiescence => {
            conduit_kernel::scheduler::AssignedConnectionTrack::Quiescence
        }
    }
}

fn source_contract_matches(
    descriptor: &LoweredPort,
    track: ConnectionTrack,
    value_kind: &KindId,
    temporal: PortTemporal,
) -> bool {
    match track {
        ConnectionTrack::AbnormalTerminal => {
            temporal == PortTemporal::Value && descriptor.abnormal_kind.as_ref() == Some(value_kind)
        }
        ConnectionTrack::Payload => {
            descriptor.value_kind == *value_kind && descriptor.temporal == temporal
        }
        ConnectionTrack::NormalClose => {
            descriptor.temporal == (PortTemporal::Flow { closes: true })
                && value_kind.as_str() == conduit_core::UNIT_INFO_ID
                && temporal == PortTemporal::Value
        }
        ConnectionTrack::Quiescence => {
            matches!(descriptor.temporal, PortTemporal::Flow { .. })
                && value_kind.as_str() == conduit_core::UNIT_INFO_ID
                && temporal == PortTemporal::Value
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    InvalidFragment,
    UnsupportedState(conduit_core::StateId),
    StateStorageExceeded,
    EmptyFragment,
    CapacityOverflow,
    ProfileCapacityExceeded {
        placement_id: PlacementId,
        direction: PortDirection,
        required: usize,
        available: usize,
    },
    DuplicatePlacement(PlacementId),
    DuplicateConnection(ConnectionId),
    DuplicatePort {
        placement_id: PlacementId,
        port_id: PlanPortId,
    },
    UnknownConnectionEndpoint(ConnectionId),
    UnknownConnectionPort(ConnectionId),
    ConnectionContractMismatch(ConnectionId),
    InvalidConnectionBudget(ConnectionId),
    InvalidRemoteConnection(ConnectionId),
    MultipleConnectionsToInput {
        placement_id: PlacementId,
        port_id: PlanPortId,
    },
    PortDirectionMismatch {
        placement_id: PlacementId,
        port_id: PlanPortId,
    },
    InvalidTerminalTransduction(PlacementId),
    UnsupportedHostCallConcurrency(PlacementId),
    ResourceBindingInvalid(PlacementId),
    /// Resource authority cannot enter the byte-valued Cord store. A future
    /// executable path must bind an already-issued local handle slot.
    ResourceAuthorityTransferUnsupported(ConnectionId),
    SignBudgetInvalid,
    SignReferenceMissing,
    SharedPoolInvalid(SharedPoolId),
    SharedPoolConsumerMissing(SharedPoolId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPort {
    pub node: NodeId,
    pub port: PortId,
    pub port_id: PlanPortId,
    pub value_kind: KindId,
    pub direction: PortDirection,
    pub temporal: conduit_core::PortTemporal,
    pub abnormal_kind: Option<KindId>,
    pub maximum_value_bytes: Option<u64>,
    /// Independent finite envelope for the endpoint's abnormal (`!`) value.
    pub maximum_abnormal_value_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredNode {
    pub node: NodeId,
    pub placement_id: PlacementId,
    pub maximum_step_fuel: u16,
    pub inputs: Vec<LoweredPort>,
    pub outputs: Vec<LoweredPort>,
    pub terminal_transductions: Vec<LoweredTerminalTransduction>,
}

/// Plan-sealed semantic terminal mapping with exact kernel port ordinals.
/// Terminal payload types remain owned by the lowered ports; this separately
/// preserves what the selected Back promises to do with terminal truth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredTerminalTransduction {
    pub input: PortId,
    pub output: PortId,
    pub cancellation_input: Option<PortId>,
    pub profile: conduit_core::TerminalTransductionProfile,
}

impl LoweredTerminalTransduction {
    pub fn assigned(&self) -> conduit_kernel::scheduler::AssignedTerminalTransduction {
        use conduit_kernel::scheduler::{
            AssignedAbnormalTransduction as Abnormal,
            AssignedCancellationTransduction as Cancellation,
            AssignedFiniteTerminalEmission as Emission, AssignedNormalCloseTransduction as Normal,
            AssignedTerminalTransduction,
        };
        let emission = |bound: &conduit_core::FiniteTerminalEmission| Emission {
            maximum_items: bound.maximum_items,
            maximum_bytes: bound.maximum_bytes,
        };
        let identity = |kind: &conduit_core::KindId| {
            conduit_core::semantic_digest("conduit/kind-identity", kind.as_str().as_bytes())
        };
        AssignedTerminalTransduction {
            input: self.input,
            output: self.output,
            normal_close: match &self.profile.normal_close {
                conduit_core::NormalCloseTransduction::NotAccepted => Normal::NotAccepted,
                conduit_core::NormalCloseTransduction::PropagateAfterDrain => {
                    Normal::PropagateAfterDrain
                }
                conduit_core::NormalCloseTransduction::Consume => Normal::Consume,
                conduit_core::NormalCloseTransduction::FlushThenPropagate(bound) => {
                    Normal::FlushThenPropagate(emission(bound))
                }
                conduit_core::NormalCloseTransduction::FlushThenPropagateWhenAllClose(bound) => {
                    Normal::FlushThenPropagateWhenAllClose(emission(bound))
                }
                conduit_core::NormalCloseTransduction::PropagateWhenAllClose => {
                    Normal::PropagateWhenAllClose
                }
                conduit_core::NormalCloseTransduction::DomainSpecific { law } => {
                    Normal::DomainSpecific { law: identity(law) }
                }
            },
            abnormal: match &self.profile.abnormal {
                conduit_core::AbnormalTerminalTransduction::NotAccepted => Abnormal::NotAccepted,
                conduit_core::AbnormalTerminalTransduction::PropagateAfterDrain => {
                    Abnormal::PropagateAfterDrain
                }
                conduit_core::AbnormalTerminalTransduction::Recover => Abnormal::Recover,
                conduit_core::AbnormalTerminalTransduction::FinalizeThenPropagate(bound) => {
                    Abnormal::FinalizeThenPropagate(emission(bound))
                }
                conduit_core::AbnormalTerminalTransduction::DomainSpecific { law } => {
                    Abnormal::DomainSpecific { law: identity(law) }
                }
            },
            cancellation: match &self.profile.cancellation {
                conduit_core::CancellationTransduction::NotCancellable => {
                    Cancellation::NotCancellable
                }
                conduit_core::CancellationTransduction::Request { disposition_kind } => {
                    Cancellation::Request {
                        input: self
                            .cancellation_input
                            .expect("checked cancellation request has one lowered input"),
                        disposition_kind: identity(disposition_kind),
                    }
                }
                conduit_core::CancellationTransduction::DomainSpecific { law } => {
                    Cancellation::DomainSpecific { law: identity(law) }
                }
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredCord {
    pub connection_id: ConnectionId,
    pub spec: CordSpec,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RemoteCordDirection {
    Egress,
    Ingress,
}

/// Exact identity binding retained outside the allocation-independent kernel.
/// The host must bind this numeric endpoint to this admitted Line before
/// trigger; the Base adapter is not allowed to choose or rewrite any fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredRemoteEndpoint {
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub connection_id: ConnectionId,
    pub source_fragment_id: FragmentId,
    pub sink_fragment_id: FragmentId,
    pub direction: RemoteCordDirection,
    pub local: LinkEndpoint,
    pub peer: LinkEndpoint,
    pub value_kind: KindId,
    pub temporal: conduit_core::PortTemporal,
    pub line: AdmittedLine,
}

/// Numeric kernel binding for one plan-sealed external Fore port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredForePort {
    pub front_port_id: PlanPortId,
    pub direction: PortDirection,
    pub track: conduit_core::ConnectionTrack,
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub value_kind: KindId,
    pub value_contract: Option<conduit_core::CheckedValueContract>,
    pub abnormal_kind: Option<KindId>,
    pub temporal: conduit_core::PortTemporal,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeValueRefusal {
    NotValueTrack,
    CordCapacity,
    Malformed(conduit_core::PrimitiveInfoRefusal),
    Constraint(conduit_core::ValueConstraintRefusal),
}

impl LoweredForePort {
    /// Revalidates one committed external value against the exact sealed Fore
    /// contract before the kernel observes it.
    pub fn validate_value(&self, canonical: &[u8]) -> Result<(), ForeValueRefusal> {
        let value_kind = match self.track {
            conduit_core::ConnectionTrack::Payload
            | conduit_core::ConnectionTrack::AbnormalTerminal => &self.value_kind,
            conduit_core::ConnectionTrack::NormalClose
            | conduit_core::ConnectionTrack::Quiescence => {
                return Err(ForeValueRefusal::NotValueTrack);
            }
        };
        if canonical.len() > self.byte_capacity as usize {
            return Err(ForeValueRefusal::CordCapacity);
        }
        if let Some(contract) = &self.value_contract {
            return contract
                .validate(canonical)
                .map_err(ForeValueRefusal::Constraint);
        }
        conduit_core::validate_primitive_info(value_kind.as_str(), canonical)
            .map_err(ForeValueRefusal::Malformed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredRoute {
    pub source_node: NodeId,
    pub source_port: PortId,
    pub range: RouteRange,
    pub targets: Vec<RouteTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredHostCall {
    pub node: NodeId,
    pub call: HostCallId,
    pub contract_id: HostCallContractId,
    pub target_kind: Option<KindId>,
    pub maximum_in_flight: u16,
    pub binding: HostCallBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredResource {
    pub node: NodeId,
    pub binding: KernelResourceBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSign {
    pub expectation: SignExpectationId,
    pub expected: ExpectedSign,
    pub target: SignExpectationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelPortIdentity {
    pub node: NodeId,
    pub direction: PortDirection,
    pub port: PortId,
    pub port_id: PlanPortId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelIdentityMap {
    pub plan_id: PlanId,
    pub fragment_id: FragmentId,
    pub placements: Vec<(NodeId, PlacementId)>,
    pub ports: Vec<KernelPortIdentity>,
    pub connections: Vec<(CordId, ConnectionId)>,
    pub remote_endpoints: Vec<(RemoteEndpointId, ConnectionId)>,
    pub fore_endpoints: Vec<KernelForeEndpointIdentity>,
    pub host_calls: Vec<(NodeId, HostCallId, HostCallContractId)>,
    pub resources: Vec<(NodeId, ResourceId, PlanResourceBinding)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelForeEndpointIdentity {
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub front_port_id: PlanPortId,
    pub direction: PortDirection,
    pub track: conduit_core::ConnectionTrack,
    pub value_kind: KindId,
}

impl KernelIdentityMap {
    pub fn placement_for_node(&self, node: NodeId) -> Option<&PlacementId> {
        self.placements
            .iter()
            .find(|(candidate, _)| *candidate == node)
            .map(|(_, placement)| placement)
    }

    pub fn node_for_placement(&self, placement: &PlacementId) -> Option<NodeId> {
        self.placements
            .iter()
            .find(|(_, candidate)| candidate == placement)
            .map(|(node, _)| *node)
    }

    pub fn connection_for_cord(&self, cord: CordId) -> Option<&ConnectionId> {
        self.connections
            .iter()
            .find(|(candidate, _)| *candidate == cord)
            .map(|(_, connection)| connection)
    }

    pub fn cord_for_connection(&self, connection: &ConnectionId) -> Option<CordId> {
        self.connections
            .iter()
            .find(|(_, candidate)| candidate == connection)
            .map(|(cord, _)| *cord)
    }

    pub fn connection_for_remote_endpoint(
        &self,
        endpoint: RemoteEndpointId,
    ) -> Option<&ConnectionId> {
        self.remote_endpoints
            .iter()
            .find(|(candidate, _)| *candidate == endpoint)
            .map(|(_, connection)| connection)
    }

    pub fn remote_endpoint_for_connection(
        &self,
        connection: &ConnectionId,
    ) -> Option<RemoteEndpointId> {
        self.remote_endpoints
            .iter()
            .find(|(_, candidate)| candidate == connection)
            .map(|(endpoint, _)| *endpoint)
    }

    pub fn port_identity(
        &self,
        node: NodeId,
        direction: PortDirection,
        port: PortId,
    ) -> Option<&KernelPortIdentity> {
        self.ports.iter().find(|identity| {
            identity.node == node && identity.direction == direction && identity.port == port
        })
    }

    pub fn port_for_identity(
        &self,
        node: NodeId,
        direction: PortDirection,
        port_id: &PlanPortId,
    ) -> Option<PortId> {
        self.ports
            .iter()
            .find(|identity| {
                identity.node == node
                    && identity.direction == direction
                    && &identity.port_id == port_id
            })
            .map(|identity| identity.port)
    }

    pub fn host_call_contract(
        &self,
        node: NodeId,
        call: HostCallId,
    ) -> Option<&HostCallContractId> {
        self.host_calls
            .iter()
            .find(|(candidate_node, candidate_call, _)| {
                *candidate_node == node && *candidate_call == call
            })
            .map(|(_, _, contract)| contract)
    }

    pub fn host_call_for_contract(
        &self,
        node: NodeId,
        contract: &HostCallContractId,
    ) -> Option<HostCallId> {
        self.host_calls
            .iter()
            .find(|(candidate_node, _, candidate_contract)| {
                *candidate_node == node && candidate_contract == contract
            })
            .map(|(_, call, _)| *call)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionIdentityError {
    WrongPlan,
    WrongActivePlay,
    WrongHost,
    UnknownNode,
    UnknownHostCall,
    UnknownRequest,
    UnknownPresentation,
    DuplicateIdentity,
    CapacityExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelHostRequestIdentity {
    pub node: NodeId,
    pub request: conduit_kernel::RequestId,
    pub call: HostCallId,
    pub contract_id: HostCallContractId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelPresentationIdentity {
    pub node: NodeId,
    pub request: conduit_kernel::RequestId,
    pub presentation_id: PresentationId,
    pub placement_id: PlacementId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelSignIdentity {
    pub sign_id: SignId,
    pub sequence: u64,
    pub node: Option<NodeId>,
    pub request: Option<conduit_kernel::RequestId>,
    pub presentation_id: Option<PresentationId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelExecutionIdentityMap {
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    host_id: HostId,
    boot_id: BootId,
    requests: Vec<KernelHostRequestIdentity>,
    presentations: Vec<KernelPresentationIdentity>,
    signs: Vec<KernelSignIdentity>,
}

impl KernelExecutionIdentityMap {
    pub fn new(
        lowered: &KernelIdentityMap,
        active_play: &ActivePlayIdentity,
        request_capacity: usize,
        presentation_capacity: usize,
        sign_capacity: usize,
    ) -> Result<Self, ExecutionIdentityError> {
        if active_play.plan_id != lowered.plan_id {
            return Err(ExecutionIdentityError::WrongPlan);
        }
        Ok(Self {
            plan_id: lowered.plan_id.clone(),
            active_play_id: active_play.active_play_id.clone(),
            host_id: active_play.host_id.clone(),
            boot_id: active_play.boot_id.clone(),
            requests: Vec::with_capacity(request_capacity),
            presentations: Vec::with_capacity(presentation_capacity),
            signs: Vec::with_capacity(sign_capacity),
        })
    }

    pub fn bind_request(
        &mut self,
        lowered: &KernelIdentityMap,
        node: NodeId,
        request: conduit_kernel::RequestId,
        call: HostCallId,
    ) -> Result<(), ExecutionIdentityError> {
        let contract_id = lowered
            .host_call_contract(node, call)
            .ok_or(ExecutionIdentityError::UnknownHostCall)?;
        if self.requests.len() >= self.requests.capacity() {
            return Err(ExecutionIdentityError::CapacityExceeded);
        }
        if self
            .requests
            .iter()
            .any(|identity| identity.node == node && identity.request == request)
        {
            return Err(ExecutionIdentityError::DuplicateIdentity);
        }
        self.requests.push(KernelHostRequestIdentity {
            node,
            request,
            call,
            contract_id: contract_id.clone(),
        });
        Ok(())
    }

    pub fn bind_presentation(
        &mut self,
        lowered: &KernelIdentityMap,
        node: NodeId,
        request: conduit_kernel::RequestId,
        presentation: &PresentationIdentity,
    ) -> Result<(), ExecutionIdentityError> {
        if presentation.active_play_id != self.active_play_id {
            return Err(ExecutionIdentityError::WrongActivePlay);
        }
        if lowered.node_for_placement(&presentation.placement_id) != Some(node) {
            return Err(ExecutionIdentityError::UnknownNode);
        }
        if self.request(node, request).is_none() {
            return Err(ExecutionIdentityError::UnknownRequest);
        }
        if self.presentations.len() >= self.presentations.capacity() {
            return Err(ExecutionIdentityError::CapacityExceeded);
        }
        if self.presentations.iter().any(|identity| {
            identity.presentation_id == presentation.presentation_id
                || (identity.node == node && identity.request == request)
        }) {
            return Err(ExecutionIdentityError::DuplicateIdentity);
        }
        self.presentations.push(KernelPresentationIdentity {
            node,
            request,
            presentation_id: presentation.presentation_id.clone(),
            placement_id: presentation.placement_id.clone(),
        });
        Ok(())
    }

    pub fn bind_sign(
        &mut self,
        sign: &SignIdentity,
        node: Option<NodeId>,
        request: Option<conduit_kernel::RequestId>,
        presentation_id: Option<&PresentationId>,
    ) -> Result<(), ExecutionIdentityError> {
        if sign.active_play_id.as_ref() != Some(&self.active_play_id) {
            return Err(ExecutionIdentityError::WrongActivePlay);
        }
        if sign.host_id != self.host_id || sign.boot_id != self.boot_id {
            return Err(ExecutionIdentityError::WrongHost);
        }
        if node.is_some() != request.is_some() {
            return Err(ExecutionIdentityError::UnknownRequest);
        }
        if let Some((node, request)) = node.zip(request) {
            if self.request(node, request).is_none() {
                return Err(ExecutionIdentityError::UnknownRequest);
            }
        }
        if let Some(presentation_id) = presentation_id {
            let presentation = self
                .presentation(presentation_id)
                .ok_or(ExecutionIdentityError::UnknownPresentation)?;
            if Some(presentation.node) != node || Some(presentation.request) != request {
                return Err(ExecutionIdentityError::UnknownPresentation);
            }
        }
        if self.signs.len() >= self.signs.capacity() {
            return Err(ExecutionIdentityError::CapacityExceeded);
        }
        if self
            .signs
            .iter()
            .any(|identity| identity.sign_id == sign.sign_id)
        {
            return Err(ExecutionIdentityError::DuplicateIdentity);
        }
        self.signs.push(KernelSignIdentity {
            sign_id: sign.sign_id.clone(),
            sequence: sign.sequence,
            node,
            request,
            presentation_id: presentation_id.cloned(),
        });
        Ok(())
    }

    pub fn request(
        &self,
        node: NodeId,
        request: conduit_kernel::RequestId,
    ) -> Option<&KernelHostRequestIdentity> {
        self.requests
            .iter()
            .find(|identity| identity.node == node && identity.request == request)
    }

    pub fn request_for_contract<'a>(
        &'a self,
        node: NodeId,
        contract: &'a HostCallContractId,
    ) -> impl Iterator<Item = &'a KernelHostRequestIdentity> + 'a {
        self.requests
            .iter()
            .filter(move |identity| identity.node == node && &identity.contract_id == contract)
    }

    pub fn presentation(
        &self,
        presentation: &PresentationId,
    ) -> Option<&KernelPresentationIdentity> {
        self.presentations
            .iter()
            .find(|identity| &identity.presentation_id == presentation)
    }

    pub fn presentation_for_request(
        &self,
        node: NodeId,
        request: conduit_kernel::RequestId,
    ) -> Option<&KernelPresentationIdentity> {
        self.presentations
            .iter()
            .find(|identity| identity.node == node && identity.request == request)
    }

    pub fn sign_identity(&self, sign: &SignId) -> Option<&KernelSignIdentity> {
        self.signs.iter().find(|identity| &identity.sign_id == sign)
    }

    pub fn sign_for_presentation(
        &self,
        presentation: &PresentationId,
    ) -> Option<&KernelSignIdentity> {
        self.signs
            .iter()
            .find(|identity| identity.presentation_id.as_ref() == Some(presentation))
    }

    pub fn allocation_capacities(&self) -> (usize, usize, usize) {
        (
            self.requests.capacity(),
            self.presentations.capacity(),
            self.signs.capacity(),
        )
    }

    pub fn lengths(&self) -> (usize, usize, usize) {
        (
            self.requests.len(),
            self.presentations.len(),
            self.signs.len(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPlanFragment {
    pub completion_policy: conduit_core::PlanCompletionPolicy,
    pub identity: KernelIdentityMap,
    pub nodes: Vec<LoweredNode>,
    pub states: Vec<LoweredState>,
    pub node_specs: Vec<NodeSpec<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>>,
    pub cords: Vec<LoweredCord>,
    pub fusions: Vec<LoweredFusion>,
    pub remote_endpoints: Vec<LoweredRemoteEndpoint>,
    pub fore_ports: Vec<LoweredForePort>,
    pub routes: Vec<LoweredRoute>,
    pub host_calls: Vec<LoweredHostCall>,
    pub resources: Vec<LoweredResource>,
    pub signs: Vec<LoweredSign>,
    pub shared_pools: Vec<LoweredSharedPool>,
    pub cord_value_slots: u16,
    pub cord_value_bytes: u32,
    pub sign_items: u16,
    pub sign_bytes: u32,
}

pub fn lower_plan_fragment(fragment: &PlanFragment) -> Result<LoweredPlanFragment, LoweringError> {
    lower_plan_fragment_for_profile(fragment, FIXED_KERNEL_STORAGE_PROFILE)
}

pub fn lower_plan_fragment_for_profile(
    fragment: &PlanFragment,
    profile: KernelStorageProfile,
) -> Result<LoweredPlanFragment, LoweringError> {
    admission::validate_fragment(fragment, profile)?;
    let mut placement_nodes = BTreeMap::new();
    let mut nodes = Vec::with_capacity(fragment.placements.len());
    let mut node_specs = Vec::with_capacity(fragment.placements.len());
    let mut identity_ports = Vec::new();
    for (node_index, placement) in fragment.placements.iter().enumerate() {
        let node = NodeId(as_u16(node_index)?);
        if placement_nodes
            .insert(placement.placement_id.clone(), node)
            .is_some()
        {
            return Err(LoweringError::DuplicatePlacement(
                placement.placement_id.clone(),
            ));
        }
        let checked_front = placement.checked_port_front();
        let inputs = lower_ports(
            node,
            &placement.placement_id,
            &placement.inputs,
            PortDirection::Input,
            checked_front.value_contracts(),
        )?;
        let outputs = lower_ports(
            node,
            &placement.placement_id,
            &placement.outputs,
            PortDirection::Output,
            checked_front.value_contracts(),
        )?;
        if inputs.len() > profile.maximum_ports_per_node() {
            return Err(LoweringError::ProfileCapacityExceeded {
                placement_id: placement.placement_id.clone(),
                direction: PortDirection::Input,
                required: inputs.len(),
                available: profile.maximum_ports_per_node(),
            });
        }
        if outputs.len() > profile.maximum_ports_per_node() {
            return Err(LoweringError::ProfileCapacityExceeded {
                placement_id: placement.placement_id.clone(),
                direction: PortDirection::Output,
                required: outputs.len(),
                available: profile.maximum_ports_per_node(),
            });
        }
        let input_cords = [None; FIXED_KERNEL_STORAGE_PORTS_PER_NODE];
        let maximum_step_fuel = 1usize
            .checked_add(inputs.len())
            .and_then(|value| value.checked_add(outputs.len()))
            .and_then(|value| value.checked_add(placement.host_calls.len()))
            .ok_or(LoweringError::CapacityOverflow)
            .and_then(as_u16)?;
        let terminal_transductions = placement
            .terminal_transductions
            .iter()
            .map(|profile| {
                let input = inputs
                    .iter()
                    .find(|port| port.port_id == profile.input_port_id)
                    .map(|port| port.port)
                    .ok_or_else(|| {
                        LoweringError::InvalidTerminalTransduction(placement.placement_id.clone())
                    })?;
                let output = outputs
                    .iter()
                    .find(|port| port.port_id == profile.output_port_id)
                    .map(|port| port.port)
                    .ok_or_else(|| {
                        LoweringError::InvalidTerminalTransduction(placement.placement_id.clone())
                    })?;
                let cancellation_input = if matches!(
                    profile.cancellation,
                    conduit_core::CancellationTransduction::Request { .. }
                ) {
                    Some(
                        inputs
                            .iter()
                            .find(|port| {
                                port.value_kind.as_str()
                                    == conduit_core::CANCELLATION_REQUEST_INFO_ID
                            })
                            .map(|port| port.port)
                            .ok_or_else(|| {
                                LoweringError::InvalidTerminalTransduction(
                                    placement.placement_id.clone(),
                                )
                            })?,
                    )
                } else {
                    None
                };
                Ok(LoweredTerminalTransduction {
                    input,
                    output,
                    cancellation_input,
                    profile: profile.clone(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        identity_ports.extend(
            inputs
                .iter()
                .chain(outputs.iter())
                .map(|port| KernelPortIdentity {
                    node,
                    direction: port.direction,
                    port: port.port,
                    port_id: port.port_id.clone(),
                }),
        );
        nodes.push(LoweredNode {
            node,
            placement_id: placement.placement_id.clone(),
            maximum_step_fuel,
            inputs,
            outputs,
            terminal_transductions: terminal_transductions.clone(),
        });
        node_specs.push(NodeSpec {
            input_cords,
            maximum_step_fuel,
        });
    }

    let mut connection_ids = BTreeSet::new();
    let mut cords = Vec::with_capacity(fragment.connections.len());
    let mut remote_endpoints = Vec::new();
    let mut value_slots = 0u16;
    let mut value_bytes = 0u32;
    for (cord_index, connection) in fragment.connections.iter().enumerate() {
        if !connection_ids.insert(connection.connection_id.clone()) {
            return Err(LoweringError::DuplicateConnection(
                connection.connection_id.clone(),
            ));
        }
        if connection.resource.is_some() {
            return Err(LoweringError::ResourceAuthorityTransferUnsupported(
                connection.connection_id.clone(),
            ));
        }
        if connection.item_capacity == 0 || connection.byte_capacity == 0 {
            return Err(LoweringError::InvalidConnectionBudget(
                connection.connection_id.clone(),
            ));
        }
        let cord = CordId(as_u16(cord_index)?);
        let source_node = placement_nodes
            .get(&connection.source_placement_id)
            .copied();
        let sink_node = placement_nodes.get(&connection.sink_placement_id).copied();
        let source_port = source_node
            .map(|node| {
                find_port(
                    &nodes[usize::from(node.0)].outputs,
                    &connection.source_port_id,
                )
                .ok_or_else(|| {
                    LoweringError::UnknownConnectionPort(connection.connection_id.clone())
                })
            })
            .transpose()?;
        let sink_port = sink_node
            .map(|node| {
                find_port(&nodes[usize::from(node.0)].inputs, &connection.sink_port_id).ok_or_else(
                    || LoweringError::UnknownConnectionPort(connection.connection_id.clone()),
                )
            })
            .transpose()?;
        if source_node.zip(source_port).is_some_and(|(node, port)| {
            let descriptor = &nodes[usize::from(node.0)].outputs[usize::from(port.0)];
            !source_contract_matches(
                descriptor,
                connection.track,
                &connection.value_kind,
                connection.temporal,
            )
        }) || sink_node.zip(sink_port).is_some_and(|(node, port)| {
            let descriptor = &nodes[usize::from(node.0)].inputs[usize::from(port.0)];
            descriptor.value_kind != connection.value_kind
                || (descriptor.temporal != connection.temporal
                    && !matches!(
                        (connection.temporal, descriptor.temporal),
                        (
                            conduit_core::PortTemporal::Flow { .. },
                            conduit_core::PortTemporal::Value
                        )
                    )
                    && !matches!(
                        (connection.temporal, descriptor.temporal),
                        (
                            conduit_core::PortTemporal::Flow { closes: true },
                            conduit_core::PortTemporal::Flow { closes: false }
                        )
                    ))
        }) {
            return Err(LoweringError::ConnectionContractMismatch(
                connection.connection_id.clone(),
            ));
        }
        let slot_start = value_slots;
        let source_value_bound = source_node.zip(source_port).and_then(|(node, port)| {
            let descriptor = &nodes[usize::from(node.0)].outputs[usize::from(port.0)];
            match connection.track {
                ConnectionTrack::AbnormalTerminal => descriptor.maximum_abnormal_value_bytes,
                ConnectionTrack::Payload => descriptor.maximum_value_bytes,
                ConnectionTrack::NormalClose | ConnectionTrack::Quiescence => None,
            }
        });
        let sink_value_bound = sink_node.zip(sink_port).and_then(|(node, port)| {
            nodes[usize::from(node.0)].inputs[usize::from(port.0)].maximum_value_bytes
        });
        if !matches!(
            connection.track,
            ConnectionTrack::NormalClose | ConnectionTrack::Quiescence
        ) && source_value_bound
            .zip(sink_value_bound)
            .is_some_and(|(source, sink)| source != sink)
        {
            return Err(LoweringError::ConnectionContractMismatch(
                connection.connection_id.clone(),
            ));
        }
        let maximum_value_bytes = if !matches!(
            connection.track,
            ConnectionTrack::NormalClose | ConnectionTrack::Quiescence
        ) {
            admitted_maximum_value_bytes(
                source_value_bound
                    .or(sink_value_bound)
                    .map(u32::try_from)
                    .transpose()
                    .map_err(|_| LoweringError::CapacityOverflow)?,
                connection.byte_capacity,
            )
        } else {
            connection.byte_capacity
        };
        value_slots = value_slots
            .checked_add(connection.item_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        value_bytes = value_bytes
            .checked_add(connection.byte_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        if let Some((sink_node, sink_port)) = sink_node.zip(sink_port) {
            let sink_slot =
                &mut node_specs[usize::from(sink_node.0)].input_cords[usize::from(sink_port.0)];
            if sink_slot.is_some() {
                return Err(LoweringError::MultipleConnectionsToInput {
                    placement_id: connection.sink_placement_id.clone(),
                    port_id: connection.sink_port_id.clone(),
                });
            }
            *sink_slot = Some(cord);
        }
        let spec = match (source_node.zip(source_port), sink_node.zip(sink_port)) {
            (Some((source_node, source_port)), Some((sink_node, sink_port))) => {
                if connection.selected_line.is_some() || !connection.admitted_lines.is_empty() {
                    return Err(LoweringError::InvalidRemoteConnection(
                        connection.connection_id.clone(),
                    ));
                }
                CordSpec::local(
                    cord,
                    (source_node, source_port),
                    (sink_node, sink_port),
                    CordCapacity {
                        slot_start,
                        item_capacity: connection.item_capacity,
                        byte_capacity: connection.byte_capacity,
                        pressure_policy: lower_pressure_policy(connection.pressure_policy),
                    },
                )
                .with_track(lower_connection_track(connection.track))
                .with_maximum_value_bytes(maximum_value_bytes)
            }
            (Some((source_node, source_port)), None) => {
                let endpoint = lower_remote_endpoints(
                    fragment,
                    connection,
                    cord,
                    RemoteCordDirection::Egress,
                    &mut remote_endpoints,
                )?;
                CordSpec::remote_egress(
                    cord,
                    (source_node, source_port),
                    endpoint,
                    CordCapacity {
                        slot_start,
                        item_capacity: connection.item_capacity,
                        byte_capacity: connection.byte_capacity,
                        pressure_policy: lower_pressure_policy(connection.pressure_policy),
                    },
                )
                .with_track(lower_connection_track(connection.track))
                .with_maximum_value_bytes(maximum_value_bytes)
            }
            (None, Some((sink_node, sink_port))) => {
                let endpoint = lower_remote_endpoints(
                    fragment,
                    connection,
                    cord,
                    RemoteCordDirection::Ingress,
                    &mut remote_endpoints,
                )?;
                CordSpec::remote_ingress(
                    cord,
                    endpoint,
                    (sink_node, sink_port),
                    CordCapacity {
                        slot_start,
                        item_capacity: connection.item_capacity,
                        byte_capacity: connection.byte_capacity,
                        pressure_policy: lower_pressure_policy(connection.pressure_policy),
                    },
                )
                .with_track(lower_connection_track(connection.track))
                .with_maximum_value_bytes(maximum_value_bytes)
            }
            (None, None) => {
                return Err(LoweringError::UnknownConnectionEndpoint(
                    connection.connection_id.clone(),
                ));
            }
        };
        cords.push(LoweredCord {
            connection_id: connection.connection_id.clone(),
            spec,
        });
    }

    let mut fore_ports = Vec::with_capacity(fragment.fore_ports.len());
    for planned in &fragment.fore_ports {
        if planned.item_capacity == 0 || planned.byte_capacity == 0 {
            return Err(LoweringError::InvalidConnectionBudget(ConnectionId::from(
                planned.front_port_id.as_str(),
            )));
        }
        let node = placement_nodes
            .get(&planned.placement_id)
            .copied()
            .ok_or_else(|| {
                LoweringError::UnknownConnectionEndpoint(ConnectionId::from(
                    planned.front_port_id.as_str(),
                ))
            })?;
        let ports = match planned.direction {
            PortDirection::Input => &nodes[usize::from(node.0)].inputs,
            PortDirection::Output => &nodes[usize::from(node.0)].outputs,
        };
        let port = find_port(ports, &planned.gear_port_id).ok_or_else(|| {
            LoweringError::UnknownConnectionPort(ConnectionId::from(planned.front_port_id.as_str()))
        })?;
        let descriptor = &ports[usize::from(port.0)];
        let descriptor_matches = match planned.track {
            ConnectionTrack::Payload => {
                descriptor.value_kind == planned.value_kind
                    && descriptor.temporal == planned.temporal
            }
            ConnectionTrack::AbnormalTerminal => {
                descriptor.abnormal_kind.as_ref() == Some(&planned.value_kind)
                    && planned.temporal == PortTemporal::Value
            }
            ConnectionTrack::NormalClose => {
                descriptor.temporal == (PortTemporal::Flow { closes: true })
                    && planned.value_kind.as_str() == conduit_core::UNIT_INFO_ID
                    && planned.temporal == PortTemporal::Value
            }
            ConnectionTrack::Quiescence => {
                matches!(descriptor.temporal, PortTemporal::Flow { .. })
                    && planned.value_kind.as_str() == conduit_core::UNIT_INFO_ID
                    && planned.temporal == PortTemporal::Value
            }
        };
        if !descriptor_matches {
            return Err(LoweringError::ConnectionContractMismatch(
                ConnectionId::from(planned.front_port_id.as_str()),
            ));
        }
        let cord = CordId(as_u16(cords.len())?);
        let endpoint = RemoteEndpointId(as_u16(remote_endpoints.len() + fore_ports.len())?);
        let capacity = CordCapacity {
            slot_start: value_slots,
            item_capacity: planned.item_capacity,
            byte_capacity: planned.byte_capacity,
            pressure_policy: lower_pressure_policy(planned.pressure_policy),
        };
        value_slots = value_slots
            .checked_add(planned.item_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        value_bytes = value_bytes
            .checked_add(planned.byte_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        let maximum_value_bytes = if matches!(
            planned.track,
            ConnectionTrack::NormalClose | ConnectionTrack::Quiescence
        ) {
            planned.byte_capacity
        } else {
            admitted_maximum_value_bytes(
                planned
                    .value_contract
                    .as_ref()
                    .map(|contract| contract.maximum_bytes),
                planned.byte_capacity,
            )
        };
        let spec = match planned.direction {
            PortDirection::Input => {
                let input = &mut node_specs[usize::from(node.0)].input_cords[usize::from(port.0)];
                if input.replace(cord).is_some() {
                    return Err(LoweringError::MultipleConnectionsToInput {
                        placement_id: planned.placement_id.clone(),
                        port_id: planned.gear_port_id.clone(),
                    });
                }
                CordSpec::remote_ingress(cord, endpoint, (node, port), capacity)
            }
            PortDirection::Output => {
                CordSpec::remote_egress(cord, (node, port), endpoint, capacity)
            }
        }
        .with_track(lower_connection_track(planned.track))
        .with_maximum_value_bytes(maximum_value_bytes);
        cords.push(LoweredCord {
            connection_id: ConnectionId::from(alloc::format!(
                "front/{}/{}/{}/{}",
                planned.direction as u8,
                planned.front_port_id.as_str(),
                planned.placement_id.as_str(),
                planned.gear_port_id.as_str(),
            )),
            spec,
        });
        fore_ports.push(LoweredForePort {
            front_port_id: planned.front_port_id.clone(),
            direction: planned.direction,
            track: planned.track,
            endpoint,
            cord,
            value_kind: planned.value_kind.clone(),
            value_contract: planned.value_contract.clone(),
            abnormal_kind: planned.abnormal_kind.clone(),
            temporal: planned.temporal,
            item_capacity: planned.item_capacity,
            byte_capacity: planned.byte_capacity,
        });
    }

    let routes = lower_routes(&cords)?;
    let mut host_calls = Vec::new();
    let mut resources = Vec::new();
    for (placement, node) in fragment.placements.iter().zip(nodes.iter()) {
        for (index, requirement) in placement.host_calls.iter().enumerate() {
            if requirement.maximum_in_flight != 1 {
                return Err(LoweringError::UnsupportedHostCallConcurrency(
                    placement.placement_id.clone(),
                ));
            }
            let call = HostCallId(as_u16(index)?);
            host_calls.push(LoweredHostCall {
                node: node.node,
                call,
                contract_id: requirement.contract_id.clone(),
                target_kind: requirement.target_kind.clone(),
                maximum_in_flight: requirement.maximum_in_flight,
                binding: HostCallBinding {
                    call,
                    maximum_input_bytes: requirement.maximum_input_bytes,
                    maximum_output_bytes: requirement.maximum_output_bytes,
                },
            });
        }
        for (index, binding) in placement.resources.iter().enumerate() {
            if binding.units == 0 {
                return Err(LoweringError::ResourceBindingInvalid(
                    placement.placement_id.clone(),
                ));
            }
            resources.push(LoweredResource {
                node: node.node,
                binding: KernelResourceBinding {
                    resource: ResourceId(as_u16(index)?),
                    units: binding.units,
                },
            });
        }
    }

    let mut signs = Vec::with_capacity(fragment.expected_sign.len());
    for (index, expected) in fragment.expected_sign.iter().enumerate() {
        let target = match expected {
            ExpectedSign::PlanFragmentReceived | ExpectedSign::PlanTerminal => {
                SignExpectationTarget::Fragment
            }
            ExpectedSign::PlacementPrepared(id) | ExpectedSign::PlacementTerminal(id) => {
                SignExpectationTarget::Node(
                    *placement_nodes
                        .get(id)
                        .ok_or(LoweringError::SignReferenceMissing)?,
                )
            }
            ExpectedSign::ConnectionTerminal(id) => SignExpectationTarget::Cord(
                cords
                    .iter()
                    .find(|cord| &cord.connection_id == id)
                    .map(|cord| cord.spec.cord)
                    .ok_or(LoweringError::SignReferenceMissing)?,
            ),
        };
        signs.push(LoweredSign {
            expectation: SignExpectationId(as_u16(index)?),
            expected: expected.clone(),
            target,
        });
    }

    let shared_pools = lower_shared_pools(fragment, &placement_nodes)?;
    let fusions = lower_fusions(fragment, &placement_nodes, &cords)?;
    let states = state::lower_states(fragment, &placement_nodes)?;

    Ok(LoweredPlanFragment {
        completion_policy: fragment.completion_policy,
        identity: KernelIdentityMap {
            plan_id: fragment.plan_id.clone(),
            fragment_id: fragment.fragment_id.clone(),
            placements: nodes
                .iter()
                .map(|node| (node.node, node.placement_id.clone()))
                .collect(),
            ports: identity_ports,
            connections: cords
                .iter()
                .map(|cord| (cord.spec.cord, cord.connection_id.clone()))
                .collect(),
            remote_endpoints: remote_endpoints
                .iter()
                .map(|item| (item.endpoint, item.connection_id.clone()))
                .collect(),
            fore_endpoints: fore_ports
                .iter()
                .map(|item| KernelForeEndpointIdentity {
                    endpoint: item.endpoint,
                    cord: item.cord,
                    front_port_id: item.front_port_id.clone(),
                    direction: item.direction,
                    track: item.track,
                    value_kind: item.value_kind.clone(),
                })
                .collect(),
            host_calls: host_calls
                .iter()
                .map(|item| (item.node, item.call, item.contract_id.clone()))
                .collect(),
            resources: resources
                .iter()
                .zip(
                    fragment
                        .placements
                        .iter()
                        .flat_map(|placement| &placement.resources),
                )
                .map(|(item, binding)| (item.node, item.binding.resource, binding.clone()))
                .collect(),
        },
        nodes,
        node_specs,
        states,
        cords,
        fusions,
        remote_endpoints,
        fore_ports,
        routes,
        host_calls,
        resources,
        signs,
        shared_pools,
        cord_value_slots: value_slots,
        cord_value_bytes: value_bytes,
        sign_items: fragment.sign_storage_budget.item_capacity,
        sign_bytes: fragment.sign_storage_budget.byte_capacity,
    })
}

fn admitted_maximum_value_bytes(semantic_bound: Option<u32>, byte_capacity: u32) -> u32 {
    semantic_bound.unwrap_or(byte_capacity).min(byte_capacity)
}

fn fragment_id_for_host(
    fragment: &PlanFragment,
    host_id: &HostId,
) -> Result<FragmentId, LoweringError> {
    fragment
        .plan_fragments
        .iter()
        .find(|commitment| &commitment.host_id == host_id)
        .map(|commitment| commitment.fragment_id.clone())
        .ok_or(LoweringError::InvalidFragment)
}

fn lower_routes(cords: &[LoweredCord]) -> Result<Vec<LoweredRoute>, LoweringError> {
    let mut grouped = BTreeMap::<(NodeId, PortId), Vec<RouteTarget>>::new();
    for cord in cords {
        if let Some((source_node, source_port)) = cord.spec.source_local() {
            grouped
                .entry((source_node, source_port))
                .or_default()
                .push(RouteTarget {
                    cord: cord.spec.cord,
                    sink: cord.spec.sink,
                });
        }
    }
    let mut next_target = 0u16;
    grouped
        .into_iter()
        .map(|((source_node, source_port), targets)| {
            let len = as_u16(targets.len())?;
            let range = RouteRange {
                start: next_target,
                len,
            };
            next_target = next_target
                .checked_add(len)
                .ok_or(LoweringError::CapacityOverflow)?;
            Ok(LoweredRoute {
                source_node,
                source_port,
                range,
                targets,
            })
        })
        .collect()
}

fn as_u16(value: usize) -> Result<u16, LoweringError> {
    u16::try_from(value).map_err(|_| LoweringError::CapacityOverflow)
}

#[cfg(test)]
mod terminal_track_tests {
    use super::*;
    use conduit_core::{kind_id, port_id};

    fn source(abnormal: Option<&str>) -> LoweredPort {
        LoweredPort {
            node: NodeId(0),
            port: PortId(0),
            port_id: port_id("out"),
            value_kind: kind_id("value/count"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: abnormal.map(kind_id),
            maximum_value_bytes: None,
            maximum_abnormal_value_bytes: None,
        }
    }

    #[test]
    fn abnormal_lowering_uses_the_exact_declared_terminal_kind() {
        let descriptor = source(Some("test/fault"));
        assert!(source_contract_matches(
            &descriptor,
            ConnectionTrack::AbnormalTerminal,
            &kind_id("test/fault"),
            PortTemporal::Value
        ));
        assert!(!source_contract_matches(
            &descriptor,
            ConnectionTrack::AbnormalTerminal,
            &kind_id("test/other-fault"),
            PortTemporal::Value
        ));
        assert!(!source_contract_matches(
            &source(None),
            ConnectionTrack::AbnormalTerminal,
            &kind_id("test/fault"),
            PortTemporal::Value
        ));
        assert!(!source_contract_matches(
            &descriptor,
            ConnectionTrack::AbnormalTerminal,
            &kind_id("test/fault"),
            PortTemporal::Flow { closes: true }
        ));
    }

    #[test]
    fn payload_lowering_retains_the_ordinary_value_contract() {
        let descriptor = source(Some("test/fault"));
        assert!(source_contract_matches(
            &descriptor,
            ConnectionTrack::Payload,
            &kind_id("value/count"),
            PortTemporal::Flow { closes: true }
        ));
        assert!(!source_contract_matches(
            &descriptor,
            ConnectionTrack::Payload,
            &kind_id("test/fault"),
            PortTemporal::Value
        ));
    }

    #[test]
    fn payload_value_maximum_is_bounded_by_the_admitted_cord_capacity() {
        assert_eq!(admitted_maximum_value_bytes(Some(256), 64), 64);
        assert_eq!(admitted_maximum_value_bytes(Some(32), 64), 32);
        assert_eq!(admitted_maximum_value_bytes(None, 64), 64);
    }

    #[test]
    fn external_fore_value_revalidates_the_sealed_constraint() {
        let port = LoweredForePort {
            front_port_id: port_id("count"),
            direction: PortDirection::Input,
            track: ConnectionTrack::Payload,
            endpoint: RemoteEndpointId(0),
            cord: CordId(0),
            value_kind: kind_id(conduit_core::COUNT_INFO_ID),
            value_contract: Some(
                conduit_core::CheckedValueContract::new(
                    kind_id(conduit_core::COUNT_INFO_ID),
                    conduit_core::COUNT_ENCODED_LEN as u32,
                    alloc::vec![conduit_core::ValueConstraint::UnsignedRange {
                        minimum: 2,
                        maximum: 4,
                        minimum_endpoint: conduit_core::IntervalEndpoint::Inclusive,
                        maximum_endpoint: conduit_core::IntervalEndpoint::Inclusive,
                    }],
                )
                .unwrap(),
            ),
            abnormal_kind: None,
            temporal: PortTemporal::Value,
            item_capacity: 1,
            byte_capacity: conduit_core::COUNT_ENCODED_LEN as u32,
        };
        assert_eq!(port.validate_value(&conduit_core::encode_count(3)), Ok(()));
        assert_eq!(
            port.validate_value(&conduit_core::encode_count(7)),
            Err(ForeValueRefusal::Constraint(
                conduit_core::ValueConstraintRefusal::UnsignedRange
            ))
        );

        let mut abnormal = port;
        abnormal.track = ConnectionTrack::AbnormalTerminal;
        assert_eq!(
            abnormal.validate_value(&conduit_core::encode_count(7)),
            Err(ForeValueRefusal::Constraint(
                conduit_core::ValueConstraintRefusal::UnsignedRange
            ))
        );
    }

    #[test]
    fn normal_close_requires_a_closable_source_and_unit_value_track() {
        let descriptor = source(Some("test/fault"));
        assert!(source_contract_matches(
            &descriptor,
            ConnectionTrack::NormalClose,
            &kind_id(conduit_core::UNIT_INFO_ID),
            PortTemporal::Value
        ));
        let mut standing = descriptor.clone();
        standing.temporal = PortTemporal::Flow { closes: false };
        assert!(!source_contract_matches(
            &standing,
            ConnectionTrack::NormalClose,
            &kind_id(conduit_core::UNIT_INFO_ID),
            PortTemporal::Value
        ));
        assert!(!source_contract_matches(
            &descriptor,
            ConnectionTrack::NormalClose,
            &kind_id("value/count"),
            PortTemporal::Value
        ));
        assert!(!source_contract_matches(
            &descriptor,
            ConnectionTrack::NormalClose,
            &kind_id(conduit_core::UNIT_INFO_ID),
            PortTemporal::Flow { closes: true }
        ));
    }
}
