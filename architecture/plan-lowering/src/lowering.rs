//! Exact pre-play-start lowering from string-identified plan facts into the
//! numeric tables consumed by `conduit-kernel`.

use alloc::vec::Vec;
use conduit_core::{
    ActivePlayId, ActivePlayIdentity, AdmittedLine, BootId, ConnectionId, ConnectionTrack,
    ExpectedSign, FaceId, FaceIdentity, FragmentId, HostCallContractId, HostId, KindId,
    LinkEndpoint, PlacementId, PlanFragment, PlanId, PortDirection, PortId as PlanPortId,
    PortTemporal, ResourceBinding as PlanResourceBinding, SignId, SignIdentity,
};
use conduit_kernel::{
    scheduler::{AssignedPressurePolicy, CordCapacity, CordSpec, NodeSpec},
    CordId, HostCallBinding, HostCallId, NodeId, PortId, RemoteEndpointId,
    ResourceBinding as KernelResourceBinding, ResourceId, RouteRange, RouteTarget,
    SignExpectationId, SignExpectationTarget,
};

mod admission;
mod materialize;
mod materialize_fragment;
mod materialize_identity;
mod owned_layout;
pub(crate) use materialize::reserved;
use materialize::{collect_exact, PlacementNodes};
pub use owned_layout::{fragment_lowering_layout, FragmentLoweringLayout};
mod fore_value;
pub use fore_value::{ForeValueRefusal, LoweredForePort};
mod fusion;
mod fusion_dag;
pub use fusion_dag::lower_pure_value_fusion_dags;
mod ports;
mod profile;
mod state;
mod step_work;
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
                && value_kind.as_str() == conduit_core::EMPTY_INFO_ID
                && temporal == PortTemporal::Value
        }
        ConnectionTrack::Quiescence => {
            matches!(descriptor.temporal, PortTemporal::Flow { .. })
                && value_kind.as_str() == conduit_core::EMPTY_INFO_ID
                && temporal == PortTemporal::Value
        }
    }
}

mod error;
pub use error::LoweringError;

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

/// One local resource Cord lowered outside the byte-valued Cord store.
///
/// `resource` identifies the already-reserved source binding. The opaque
/// bearer remains in the Host capability table; this table only authorizes an
/// exact move/shared acceptance between the two plan nodes and ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredResourceCord {
    pub connection_id: ConnectionId,
    pub source_node: NodeId,
    pub source_port: PortId,
    pub sink_node: NodeId,
    pub sink_port: PortId,
    pub resource: ResourceId,
    pub contract: conduit_core::ResourcePortContract,
    pub binding: PlanResourceBinding,
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
    pub resource_connections: Vec<(NodeId, ResourceId, ConnectionId)>,
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
    UnknownFace,
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
pub struct KernelFaceIdentity {
    pub node: NodeId,
    pub request: conduit_kernel::RequestId,
    pub face_id: FaceId,
    pub placement_id: PlacementId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelSignIdentity {
    pub sign_id: SignId,
    pub sequence: u64,
    pub node: Option<NodeId>,
    pub request: Option<conduit_kernel::RequestId>,
    pub face_id: Option<FaceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelExecutionIdentityMap {
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    host_id: HostId,
    boot_id: BootId,
    requests: Vec<KernelHostRequestIdentity>,
    faces: Vec<KernelFaceIdentity>,
    signs: Vec<KernelSignIdentity>,
}

impl KernelExecutionIdentityMap {
    pub fn new(
        lowered: &KernelIdentityMap,
        active_play: &ActivePlayIdentity,
        request_capacity: usize,
        face_capacity: usize,
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
            faces: Vec::with_capacity(face_capacity),
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

    pub fn bind_face(
        &mut self,
        lowered: &KernelIdentityMap,
        node: NodeId,
        request: conduit_kernel::RequestId,
        face: &FaceIdentity,
    ) -> Result<(), ExecutionIdentityError> {
        if face.active_play_id != self.active_play_id {
            return Err(ExecutionIdentityError::WrongActivePlay);
        }
        if lowered.node_for_placement(&face.placement_id) != Some(node) {
            return Err(ExecutionIdentityError::UnknownNode);
        }
        if self.request(node, request).is_none() {
            return Err(ExecutionIdentityError::UnknownRequest);
        }
        if self.faces.len() >= self.faces.capacity() {
            return Err(ExecutionIdentityError::CapacityExceeded);
        }
        if self.faces.iter().any(|identity| {
            identity.face_id == face.face_id
                || (identity.node == node && identity.request == request)
        }) {
            return Err(ExecutionIdentityError::DuplicateIdentity);
        }
        self.faces.push(KernelFaceIdentity {
            node,
            request,
            face_id: face.face_id.clone(),
            placement_id: face.placement_id.clone(),
        });
        Ok(())
    }

    pub fn bind_sign(
        &mut self,
        sign: &SignIdentity,
        node: Option<NodeId>,
        request: Option<conduit_kernel::RequestId>,
        face_id: Option<&FaceId>,
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
        if let Some(face_id) = face_id {
            let face = self
                .face(face_id)
                .ok_or(ExecutionIdentityError::UnknownFace)?;
            if Some(face.node) != node || Some(face.request) != request {
                return Err(ExecutionIdentityError::UnknownFace);
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
            face_id: face_id.cloned(),
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

    pub fn face(&self, face: &FaceId) -> Option<&KernelFaceIdentity> {
        self.faces.iter().find(|identity| &identity.face_id == face)
    }

    pub fn face_for_request(
        &self,
        node: NodeId,
        request: conduit_kernel::RequestId,
    ) -> Option<&KernelFaceIdentity> {
        self.faces
            .iter()
            .find(|identity| identity.node == node && identity.request == request)
    }

    pub fn sign_identity(&self, sign: &SignId) -> Option<&KernelSignIdentity> {
        self.signs.iter().find(|identity| &identity.sign_id == sign)
    }

    pub fn sign_for_face(&self, face: &FaceId) -> Option<&KernelSignIdentity> {
        self.signs
            .iter()
            .find(|identity| identity.face_id.as_ref() == Some(face))
    }

    pub fn allocation_capacities(&self) -> (usize, usize, usize) {
        (
            self.requests.capacity(),
            self.faces.capacity(),
            self.signs.capacity(),
        )
    }

    pub fn lengths(&self) -> (usize, usize, usize) {
        (self.requests.len(), self.faces.len(), self.signs.len())
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
    pub resource_cords: Vec<LoweredResourceCord>,
    pub signs: Vec<LoweredSign>,
    pub shared_pools: Vec<LoweredSharedPool>,
    pub cord_value_slots: u16,
    pub cord_value_bytes: u32,
    pub sign_items: u16,
    pub sign_bytes: u32,
}

/// Whole-Plan entry point. The legacy fragment-only function cannot carry
/// activation truth and therefore remains only for activation-free callers.
pub fn lower_plan_fragment_from_plan(
    plan: &conduit_core::Plan,
    fragment_id: &FragmentId,
) -> Result<
    (
        LoweredPlanFragment,
        crate::activation_fragment::LoweredFragmentActivations,
    ),
    LoweringError,
> {
    let activations = crate::activation_fragment::lower_fragment_activations(plan, fragment_id)
        .map_err(|_| LoweringError::InvalidFragment)?;
    Ok((
        lower_plan_fragment_for_profile_from_plan(plan, fragment_id, FIXED_KERNEL_STORAGE_PROFILE)?,
        activations,
    ))
}

/// Lowers fragment-local kernel facts only. This API cannot establish whether
/// a whole Plan owns activation coordinators and must never be used by new
/// target installation code. Activation-aware installation must use
/// [`lower_plan_fragment_from_plan`]. Kept temporarily for existing
/// activation-free target callers pending their reviewed migration.
pub fn lower_plan_fragment(fragment: &PlanFragment) -> Result<LoweredPlanFragment, LoweringError> {
    lower_plan_fragment_for_profile(fragment, FIXED_KERNEL_STORAGE_PROFILE)
}

pub fn lower_plan_fragment_for_profile(
    fragment: &PlanFragment,
    profile: KernelStorageProfile,
) -> Result<LoweredPlanFragment, LoweringError> {
    admission::validate_fragment(fragment, profile)?;
    lower_verified_fragment_for_profile(fragment, profile)
}

/// Lower a fragment whose identity is committed by the complete sealed Plan.
/// This entrance is required when the PlanId also commits activation entries.
pub fn lower_plan_fragment_for_profile_from_plan(
    plan: &conduit_core::Plan,
    fragment_id: &FragmentId,
    profile: KernelStorageProfile,
) -> Result<LoweredPlanFragment, LoweringError> {
    if !conduit_core::verify_plan(plan) {
        return Err(LoweringError::InvalidFragment);
    }
    let fragment = plan
        .fragments
        .iter()
        .find(|part| &part.fragment_id == fragment_id)
        .ok_or(LoweringError::InvalidFragment)?;
    admission::validate_verified_fragment(fragment, profile)?;
    lower_verified_fragment_for_profile(fragment, profile)
}

fn lower_verified_fragment_for_profile(
    fragment: &PlanFragment,
    profile: KernelStorageProfile,
) -> Result<LoweredPlanFragment, LoweringError> {
    materialize_fragment::lower(fragment, profile)
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
    let mut routes: Vec<LoweredRoute> = reserved(cords.len())?;
    for cord in cords {
        let Some((source_node, source_port)) = cord.spec.source_local() else {
            continue;
        };
        if routes
            .iter()
            .any(|r| r.source_node == source_node && r.source_port == source_port)
        {
            continue;
        }
        let matching = || {
            cords
                .iter()
                .filter(|c| c.spec.source_local() == Some((source_node, source_port)))
        };
        let mut targets = reserved(matching().count())?;
        targets.extend(matching().map(|c| RouteTarget {
            cord: c.spec.cord,
            sink: c.spec.sink,
        }));
        routes.push(LoweredRoute {
            source_node,
            source_port,
            range: RouteRange {
                start: 0,
                len: as_u16(targets.len())?,
            },
            targets,
        });
    }
    routes.sort_unstable_by_key(|route| (route.source_node, route.source_port));
    let mut next_target = 0u16;
    for route in &mut routes {
        route.range.start = next_target;
        next_target = next_target
            .checked_add(route.range.len)
            .ok_or(LoweringError::CapacityOverflow)?;
    }
    Ok(routes)
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
            quantity_profile: conduit_core::QuantityValueProfile::Decimal,
            value_contract: Some(
                conduit_core::CheckedValueContract::new(
                    kind_id(conduit_core::COUNT_INFO_ID),
                    conduit_core::COUNT_ENCODED_LEN as u32,
                    alloc::vec![conduit_core::ValueConstraint::UnsignedRange {
                        minimum: Some(2),
                        maximum: Some(4),
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
            selected_line: None,
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
            &kind_id(conduit_core::EMPTY_INFO_ID),
            PortTemporal::Value
        ));
        let mut standing = descriptor.clone();
        standing.temporal = PortTemporal::Flow { closes: false };
        assert!(!source_contract_matches(
            &standing,
            ConnectionTrack::NormalClose,
            &kind_id(conduit_core::EMPTY_INFO_ID),
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
            &kind_id(conduit_core::EMPTY_INFO_ID),
            PortTemporal::Flow { closes: true }
        ));
    }
}
