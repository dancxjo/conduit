use crate::child::{BoundaryEndpoint, ChildKernel, ChildTerminalError, ChildTransportError};
use crate::{KernelCompositeDefinition, KernelOperationRegistry};
use conduit_core::{
    bind_active_play, semantic_digest, ActivePlayId, AuthorityBinding, ConnectionId,
    HostCallRequirement, HostId, Plan, PortDirection, PortId, PreparationHostIdentity,
    ResourceBinding, ValuePayload,
};
use conduit_kernel::scheduler::{HostCallRequest, RemoteIngressOutcome, SchedulerStatus};
use conduit_kernel::RemoteTerminalDisposition;
use conduit_kernel::{HostCallId, HostCallOutcome, KernelEvent, NodeId, RemoteEndpointId};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, LoweredPlanFragment, LoweringError, RemoteCordDirection,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelCompositePreparation {
    plan: Plan,
    children: BTreeMap<HostId, LoweredPlanFragment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelCompositeError {
    Empty,
    DuplicateChild(HostId),
    Lowering {
        child: HostId,
        error: LoweringError,
    },
    InvalidBoundary(String),
    InvalidHostCallToken,
    HostCallDispatchMismatch,
    HostCallOutputExceeded,
    StaleHostCallChild,
    ChildRefused {
        child: HostId,
        reason: String,
    },
    Execution {
        child: HostId,
        reason: String,
    },
    UnknownFront,
    StaleChild(HostId),
    MalformedBoundary(PortId),
    InvalidLifecycle,
    CancellationRefused {
        failed_children: usize,
    },
    InternalTransport {
        link: usize,
        reason: ChildTransportError,
    },
    Terminal(ChildTerminalError),
}

impl core::fmt::Display for KernelCompositeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for KernelCompositeError {}

impl KernelCompositePreparation {
    pub fn prepare(plan: Plan) -> Result<Self, KernelCompositeError> {
        if plan.fragments.is_empty() {
            return Err(KernelCompositeError::Empty);
        }
        let mut children = BTreeMap::new();
        for fragment in &plan.fragments {
            let child = fragment.host_id.clone();
            let lowered =
                lower_plan_fragment(fragment).map_err(|error| KernelCompositeError::Lowering {
                    child: child.clone(),
                    error,
                })?;
            if children.insert(child.clone(), lowered).is_some() {
                return Err(KernelCompositeError::DuplicateChild(child));
            }
        }
        Ok(Self { plan, children })
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    pub fn child(&self, host_id: &HostId) -> Option<&LoweredPlanFragment> {
        self.children.get(host_id)
    }

    pub fn children(&self) -> impl ExactSizeIterator<Item = (&HostId, &LoweredPlanFragment)> {
        self.children.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FaceRoute {
    child: HostId,
    direction: PortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InternalLink {
    connection_id: ConnectionId,
    source_child: HostId,
    source_endpoint: RemoteEndpointId,
    source_cord: conduit_kernel::CordId,
    sink_child: HostId,
    sink_endpoint: RemoteEndpointId,
    sink_cord: conduit_kernel::CordId,
    closed: bool,
    transfer: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelCompositeHostRequest {
    pub dispatch_token: u64,
}

/// Borrowed adapter-facing fields for one live Host Call dispatch.
#[derive(Debug, Clone, Copy)]
pub struct KernelCompositeHostRequestView<'a> {
    pub child: &'a HostId,
    pub request: &'a HostCallRequest,
    /// Commitment to the selected fragment, placement, call and Host Call
    /// contract. It remains owned by the prepared outstanding slot.
    pub obligation_identity: &'a [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelCompositeHostCallObligation {
    pub host: PreparationHostIdentity,
    pub requirement: HostCallRequirement,
    pub resources: Vec<ResourceBinding>,
    pub authorities: Vec<AuthorityBinding>,
}

/// A dispatch admitted against the exact selected host identity, resources and grants.
/// Its fields are private so an adapter cannot manufacture admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedKernelCompositeHostRequest {
    dispatch_token: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutstandingHostCall {
    token: u64,
    child_index: usize,
    request: HostCallRequest,
    obligation_identity: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelCompositeStatus {
    Active,
    Complete,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelCompositeTerminal {
    Normal,
    Abnormal,
}

pub struct KernelCompositeHost {
    definition: KernelCompositeDefinition,
    children: BTreeMap<HostId, ChildKernel>,
    fronts: BTreeMap<PortId, FaceRoute>,
    links: Vec<InternalLink>,
    active_plays: BTreeMap<HostId, ActivePlayId>,
    started: bool,
    cancelled: bool,
    host_call_obligations:
        BTreeMap<(HostId, NodeId, HostCallId), ([u8; 32], KernelCompositeHostCallObligation)>,
    outstanding_host_calls: Vec<Option<OutstandingHostCall>>,
    next_dispatch_token: u64,
    cancellation_failures: Vec<(HostId, String)>,
}

impl KernelCompositeHost {
    pub(crate) fn has_exact_definition(&self, definition: &KernelCompositeDefinition) -> bool {
        &self.definition == definition
    }

    pub fn prepare(
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, KernelCompositeError> {
        let preparation = KernelCompositePreparation::prepare(definition.internal_plan.clone())?;
        let mut child_boundaries = BTreeMap::<HostId, Vec<BoundaryEndpoint>>::new();
        let mut fronts = BTreeMap::new();
        for front in definition
            .boundary
            .input_fronts
            .iter()
            .chain(&definition.boundary.output_fronts)
        {
            if fronts.contains_key(&front.external_port.port_id) {
                return Err(KernelCompositeError::InvalidBoundary(format!(
                    "duplicate external front '{}'",
                    front.external_port.port_id.as_str()
                )));
            }
            let lowered = preparation
                .child(&front.internal_child)
                .ok_or_else(|| KernelCompositeError::StaleChild(front.internal_child.clone()))?;
            let node = lowered
                .identity
                .node_for_placement(&front.internal_placement_id)
                .ok_or_else(|| invalid_front(&front.external_port.port_id, "missing placement"))?;
            lowered
                .identity
                .port_for_identity(node, front.external_port.direction, &front.internal_port_id)
                .ok_or_else(|| {
                    invalid_front(&front.external_port.port_id, "missing internal port")
                })?;
            let existing = lowered.fore_ports.iter().find(|item| {
                item.front_port_id == front.external_port.port_id
                    && item.direction == front.external_port.direction
            });
            let boundary_count = child_boundaries
                .get(&front.internal_child)
                .map_or(0, Vec::len);
            let (endpoint, cord, already_lowered) = if let Some(existing) = existing {
                (existing.endpoint, existing.cord, true)
            } else {
                (
                    RemoteEndpointId(
                        u16::try_from(lowered.remote_endpoints.len() + boundary_count).map_err(
                            |_| invalid_front(&front.external_port.port_id, "endpoint overflow"),
                        )?,
                    ),
                    conduit_kernel::CordId(
                        u16::try_from(lowered.cords.len() + boundary_count).map_err(|_| {
                            invalid_front(&front.external_port.port_id, "Cord overflow")
                        })?,
                    ),
                    false,
                )
            };
            child_boundaries
                .entry(front.internal_child.clone())
                .or_default()
                .push(BoundaryEndpoint {
                    external_port_id: front.external_port.port_id.clone(),
                    internal_port_id: front.internal_port_id.clone(),
                    endpoint,
                    cord,
                    direction: front.external_port.direction,
                    value_kind: front.external_port.value_kind.clone(),
                    abnormal_kind: front.external_port.abnormal_kind.clone(),
                    item_capacity: definition.external_capability.limits.max_queue_items,
                    byte_capacity: definition.external_capability.limits.max_queue_bytes,
                    already_lowered,
                });
            fronts.insert(
                front.external_port.port_id.clone(),
                FaceRoute {
                    child: front.internal_child.clone(),
                    direction: front.external_port.direction,
                },
            );
        }

        let links = internal_links(&preparation)?;
        let mut host_call_obligations = BTreeMap::new();
        for (child, lowered) in preparation.children() {
            for (node, call, contract) in &lowered.identity.host_calls {
                let placement_id = lowered.identity.placement_for_node(*node).ok_or_else(|| {
                    KernelCompositeError::InvalidBoundary("Host Call owner is absent".into())
                })?;
                let fragment = definition
                    .internal_plan
                    .fragments
                    .iter()
                    .find(|fragment| fragment.host_id == *child)
                    .ok_or_else(|| KernelCompositeError::StaleChild(child.clone()))?;
                let placement = fragment
                    .placements
                    .iter()
                    .find(|placement| placement.placement_id == *placement_id)
                    .ok_or_else(|| {
                        KernelCompositeError::InvalidBoundary(
                            "Host Call placement is absent".into(),
                        )
                    })?;
                let requirement = placement
                    .host_calls
                    .iter()
                    .find(|item| item.contract_id == *contract)
                    .ok_or_else(|| {
                        KernelCompositeError::InvalidBoundary(
                            "lowered Host Call contract is absent from its selected placement"
                                .into(),
                        )
                    })?;
                let obligation = KernelCompositeHostCallObligation {
                    host: PreparationHostIdentity {
                        host_id: fragment.host_id.clone(),
                        boot_id: fragment.boot_id.clone(),
                        offer_generation: fragment.offer_generation,
                    },
                    requirement: requirement.clone(),
                    resources: placement.resources.clone(),
                    authorities: placement
                        .authority
                        .iter()
                        .filter(|grant| grant.host_call_contract_id == *contract)
                        .cloned()
                        .collect(),
                };
                host_call_obligations.insert(
                    (child.clone(), *node, *call),
                    (
                        host_call_obligation_identity(
                            lowered.identity.plan_id.as_str(),
                            lowered.identity.fragment_id.as_str(),
                            placement.placement_id.as_str(),
                            *call,
                            contract.as_str(),
                        ),
                        obligation,
                    ),
                );
            }
        }
        let mut children = BTreeMap::new();
        for fragment in &definition.internal_plan.fragments {
            let child = fragment.host_id.clone();
            let lowered = preparation
                .child(&child)
                .cloned()
                .ok_or_else(|| KernelCompositeError::StaleChild(child.clone()))?;
            let kernel = ChildKernel::prepare(
                fragment,
                lowered,
                child_boundaries.remove(&child).unwrap_or_default(),
                registry,
            )
            .map_err(|reason| KernelCompositeError::ChildRefused {
                child: child.clone(),
                reason,
            })?;
            children.insert(child, kernel);
        }
        let outstanding_host_call_bound = host_call_obligations
            .values()
            .map(|(_, obligation)| usize::from(obligation.requirement.maximum_in_flight))
            .sum();
        let child_count = children.len();
        let active_plays = definition
            .internal_plan
            .fragments
            .iter()
            .map(|fragment| {
                (
                    fragment.host_id.clone(),
                    bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0)
                        .active_play_id,
                )
            })
            .collect();
        Ok(Self {
            definition,
            children,
            fronts,
            links,
            active_plays,
            started: false,
            cancelled: false,
            host_call_obligations,
            outstanding_host_calls: (0..outstanding_host_call_bound).map(|_| None).collect(),
            next_dispatch_token: 0,
            cancellation_failures: Vec::with_capacity(child_count),
        })
    }

    pub fn definition(&self) -> &KernelCompositeDefinition {
        &self.definition
    }

    pub fn internal_transfer_capacities(&self) -> (usize, usize) {
        (
            self.links.capacity(),
            self.links.iter().map(|link| link.transfer.capacity()).sum(),
        )
    }

    pub fn start(&mut self) -> Result<&BTreeMap<HostId, ActivePlayId>, KernelCompositeError> {
        if self.started || self.cancelled {
            return Err(KernelCompositeError::InvalidLifecycle);
        }
        self.started = true;
        Ok(&self.active_plays)
    }

    pub fn active_plays(&self) -> &BTreeMap<HostId, ActivePlayId> {
        &self.active_plays
    }

    pub fn admit_input(
        &mut self,
        port_id: &PortId,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<RemoteIngressOutcome, KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Input)
            .ok_or(KernelCompositeError::UnknownFront)?;
        let child = &route.child;
        self.children
            .get_mut(child)
            .ok_or_else(|| KernelCompositeError::StaleChild(child.clone()))?
            .admit_boundary(port_id, sequence, value)
            .map_err(|_| KernelCompositeError::MalformedBoundary(port_id.clone()))
    }

    pub fn close_input(&mut self, port_id: &PortId) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Input)
            .ok_or(KernelCompositeError::UnknownFront)?;
        let child = &route.child;
        self.children
            .get_mut(child)
            .ok_or_else(|| KernelCompositeError::StaleChild(child.clone()))?
            .close_boundary(port_id)
            .map_err(|reason| execution(child, reason))
    }

    pub fn close_input_abnormal(
        &mut self,
        port_id: &PortId,
        terminal: &ValuePayload,
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Input)
            .ok_or(KernelCompositeError::UnknownFront)?;
        let child = &route.child;
        self.children
            .get_mut(child)
            .ok_or_else(|| KernelCompositeError::StaleChild(child.clone()))?
            .close_boundary_abnormal(port_id, terminal)
            .map_err(|reason| execution(child, reason))
    }

    pub fn output(
        &mut self,
        port_id: &PortId,
    ) -> Result<Option<(u64, ValuePayload)>, KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Output)
            .ok_or(KernelCompositeError::UnknownFront)?;
        self.children
            .get_mut(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .boundary_output(port_id)
            .map_err(|reason| execution(&route.child, reason))
    }

    pub fn output_into(
        &mut self,
        port_id: &PortId,
        output: &mut ValuePayload,
    ) -> Result<Option<u64>, KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Output)
            .ok_or(KernelCompositeError::UnknownFront)?;
        self.children
            .get_mut(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .boundary_output_into(port_id, output)
            .map_err(|reason| execution(&route.child, reason))
    }

    pub fn complete_output(
        &mut self,
        port_id: &PortId,
        sequence: u64,
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let route = self
            .fronts
            .get(port_id)
            .filter(|route| route.direction == PortDirection::Output)
            .ok_or(KernelCompositeError::UnknownFront)?;
        self.children
            .get_mut(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .deliver_boundary(port_id, sequence)
            .map_err(|reason| execution(&route.child, reason))
    }

    pub fn output_terminal_into(
        &self,
        port_id: &PortId,
        abnormal: &mut ValuePayload,
    ) -> Result<Option<KernelCompositeTerminal>, KernelCompositeError> {
        self.require_started()?;
        let route = self.front(port_id, PortDirection::Output)?;
        self.children
            .get(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .boundary_terminal_into(port_id, abnormal)
            .map(|terminal| {
                terminal.map(|terminal| match terminal {
                    RemoteTerminalDisposition::NormalClose => KernelCompositeTerminal::Normal,
                    RemoteTerminalDisposition::Abnormal => KernelCompositeTerminal::Abnormal,
                })
            })
            .map_err(KernelCompositeError::Terminal)
    }

    pub fn next_host_request(&mut self) -> Option<KernelCompositeHostRequest> {
        if !self.started || self.cancelled {
            return None;
        }
        let slot = self
            .outstanding_host_calls
            .iter()
            .position(Option::is_none)?;
        let next_dispatch_token = self.next_dispatch_token.checked_add(1)?;
        let surfaced = self
            .children
            .iter_mut()
            .enumerate()
            .find_map(|(index, (_, kernel))| {
                kernel.next_host_request().map(|request| (index, request))
            })?;
        let (child_index, request) = surfaced;
        let child = self.children.keys().nth(child_index)?;
        let obligation_identity = self
            .host_call_obligations
            .iter()
            .find(|((host, node, call), _)| {
                host == child && *node == request.node && *call == request.call
            })
            .map(|(_, (identity, _))| *identity)
            .expect("lowered Host Call has a sealed obligation");
        let dispatch_token = self.next_dispatch_token;
        self.next_dispatch_token = next_dispatch_token;
        self.outstanding_host_calls[slot] = Some(OutstandingHostCall {
            token: dispatch_token,
            child_index,
            request,
            obligation_identity,
        });
        Some(KernelCompositeHostRequest { dispatch_token })
    }

    pub fn host_request_view(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<KernelCompositeHostRequestView<'_>, KernelCompositeError> {
        let outstanding = self.outstanding_host_call(request.dispatch_token)?;
        let child = self.child_id(outstanding.child_index)?;
        Ok(KernelCompositeHostRequestView {
            child,
            request: &outstanding.request,
            obligation_identity: &outstanding.obligation_identity,
        })
    }

    pub fn admitted_host_request_view(
        &self,
        request: &AdmittedKernelCompositeHostRequest,
    ) -> Result<KernelCompositeHostRequestView<'_>, KernelCompositeError> {
        let outstanding = self.outstanding_host_call(request.dispatch_token)?;
        let child = self.child_id(outstanding.child_index)?;
        Ok(KernelCompositeHostRequestView {
            child,
            request: &outstanding.request,
            obligation_identity: &outstanding.obligation_identity,
        })
    }

    pub fn host_request_obligation(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<&KernelCompositeHostCallObligation, KernelCompositeError> {
        let request = self.outstanding_host_call(request.dispatch_token)?;
        let child = self.child_id(request.child_index)?;
        self.host_call_obligations
            .iter()
            .find(|((host, node, call), _)| {
                host == child && *node == request.request.node && *call == request.request.call
            })
            .map(|(_, (_, obligation))| obligation)
            .ok_or_else(|| {
                KernelCompositeError::InvalidBoundary("Host Call has no selected obligation".into())
            })
    }

    pub fn admit_host_request(
        &self,
        request: &KernelCompositeHostRequest,
        host: &PreparationHostIdentity,
        resources: &[ResourceBinding],
        authorities: &[AuthorityBinding],
    ) -> Result<AdmittedKernelCompositeHostRequest, KernelCompositeError> {
        let exact = self.host_request_obligation(request)?;
        if !dispatch_matches(exact, host, resources, authorities) {
            return Err(KernelCompositeError::HostCallDispatchMismatch);
        }
        Ok(AdmittedKernelCompositeHostRequest {
            dispatch_token: request.dispatch_token,
        })
    }

    pub fn complete_host_call(
        &mut self,
        admitted: &AdmittedKernelCompositeHostRequest,
        outcome: HostCallOutcome,
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let slot = self.outstanding_host_call_index(admitted.dispatch_token)?;
        let (child_index, request) = {
            let outstanding = self.outstanding_host_calls[slot]
                .as_ref()
                .expect("resolved Host Call slot is occupied");
            (outstanding.child_index, outstanding.request)
        };
        self.children
            .values_mut()
            .nth(child_index)
            .ok_or(KernelCompositeError::StaleHostCallChild)?
            .complete_host_call(request.node, request.request, outcome)
            .map_err(KernelCompositeError::InvalidBoundary)?;
        self.outstanding_host_calls[slot] = None;
        Ok(())
    }

    /// Resolve the exact admitted input for a surfaced Host Call.
    pub fn host_request_input(
        &self,
        admitted: &AdmittedKernelCompositeHostRequest,
    ) -> Result<&[u8], KernelCompositeError> {
        let request = self.outstanding_host_call(admitted.dispatch_token)?;
        let child = self.child_id(request.child_index)?;
        self.children
            .get(child)
            .ok_or(KernelCompositeError::StaleHostCallChild)?
            .host_value(request.request.input.value)
            .map_err(|reason| execution(child, reason))
    }

    /// Store a bounded adapter result in the owning child and complete its call.
    pub fn complete_host_call_bytes(
        &mut self,
        admitted: &AdmittedKernelCompositeHostRequest,
        bytes: &[u8],
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let slot = self.outstanding_host_call_index(admitted.dispatch_token)?;
        let (child_index, request, maximum_output_bytes) = {
            let outstanding = self.outstanding_host_calls[slot]
                .as_ref()
                .expect("resolved Host Call slot is occupied");
            let child = self.child_id(outstanding.child_index)?;
            let obligation = self
                .host_call_obligations
                .iter()
                .find(|((host, node, call), _)| {
                    host == child
                        && *node == outstanding.request.node
                        && *call == outstanding.request.call
                })
                .map(|(_, (_, obligation))| obligation)
                .ok_or_else(|| {
                    KernelCompositeError::InvalidBoundary(
                        "Host Call has no selected obligation".into(),
                    )
                })?;
            (
                outstanding.child_index,
                outstanding.request,
                obligation.requirement.maximum_output_bytes,
            )
        };
        if bytes.len() > maximum_output_bytes as usize {
            return Err(KernelCompositeError::HostCallOutputExceeded);
        }
        let child = self
            .children
            .values_mut()
            .nth(child_index)
            .ok_or(KernelCompositeError::StaleHostCallChild)?;
        let value = child
            .store_host_value(bytes)
            .map_err(KernelCompositeError::InvalidBoundary)?;
        let output = conduit_kernel::BoundedValueRef::new(value, bytes.len() as u32)
            .map_err(|error| KernelCompositeError::InvalidBoundary(format!("{error:?}")))?;
        child
            .complete_host_call(
                request.node,
                request.request,
                conduit_kernel::HostCallOutcome {
                    disposition: conduit_kernel::HostCallDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            )
            .map_err(KernelCompositeError::InvalidBoundary)?;
        self.outstanding_host_calls[slot] = None;
        Ok(())
    }

    fn outstanding_host_call_index(&self, token: u64) -> Result<usize, KernelCompositeError> {
        outstanding_host_call_index(&self.outstanding_host_calls, token)
    }

    fn outstanding_host_call(
        &self,
        token: u64,
    ) -> Result<&OutstandingHostCall, KernelCompositeError> {
        let slot = self.outstanding_host_call_index(token)?;
        Ok(self.outstanding_host_calls[slot]
            .as_ref()
            .expect("resolved Host Call slot is occupied"))
    }

    fn child_id(&self, index: usize) -> Result<&HostId, KernelCompositeError> {
        self.children
            .keys()
            .nth(index)
            .ok_or(KernelCompositeError::StaleHostCallChild)
    }

    pub fn step(&mut self) -> Result<KernelCompositeStatus, KernelCompositeError> {
        if self.cancelled {
            return Ok(KernelCompositeStatus::Cancelled);
        }
        self.require_started()?;
        for (child, kernel) in &mut self.children {
            kernel.step().map_err(|reason| execution(child, reason))?;
        }
        self.pump_internal()?;
        if self
            .children
            .values()
            .all(|child| child.status() == SchedulerStatus::Drained)
            && self.links.iter().all(|link| link.closed)
        {
            Ok(KernelCompositeStatus::Complete)
        } else {
            Ok(KernelCompositeStatus::Active)
        }
    }

    pub fn cancel(&mut self) -> Result<(), KernelCompositeError> {
        // Cancellation consumes adapter authority even if a child later
        // reports mechanism trouble while cancelling.
        self.cancelled = true;
        self.outstanding_host_calls.fill(None);
        self.cancellation_failures.clear();
        for (child, kernel) in &mut self.children {
            if let Err(reason) = kernel.cancel() {
                self.cancellation_failures.push((child.clone(), reason));
            }
        }
        if self.cancellation_failures.is_empty() {
            Ok(())
        } else {
            Err(KernelCompositeError::CancellationRefused {
                failed_children: self.cancellation_failures.len(),
            })
        }
    }

    pub fn cancellation_failures(&self) -> &[(HostId, String)] {
        &self.cancellation_failures
    }

    pub fn signs(&self) -> BTreeMap<HostId, Vec<KernelEvent>> {
        self.children
            .iter()
            .map(|(child, kernel)| (child.clone(), kernel.signs()))
            .collect()
    }

    fn front(
        &self,
        port_id: &PortId,
        direction: PortDirection,
    ) -> Result<&FaceRoute, KernelCompositeError> {
        self.fronts
            .get(port_id)
            .filter(|route| route.direction == direction)
            .ok_or(KernelCompositeError::UnknownFront)
    }

    fn require_started(&self) -> Result<(), KernelCompositeError> {
        if self.started && !self.cancelled {
            Ok(())
        } else {
            Err(KernelCompositeError::InvalidLifecycle)
        }
    }

    fn pump_internal(&mut self) -> Result<(), KernelCompositeError> {
        let (children, links) = (&mut self.children, &mut self.links);
        for (link_index, link) in links.iter_mut().enumerate() {
            if link.closed {
                continue;
            }
            let offer = children
                .get_mut(&link.source_child)
                .ok_or_else(|| KernelCompositeError::StaleChild(link.source_child.clone()))?
                .remote_offer_into(link.source_endpoint, link.source_cord, &mut link.transfer)
                .map_err(|reason| KernelCompositeError::InternalTransport {
                    link: link_index,
                    reason,
                })?;
            if let Some(sequence) = offer {
                let accepted = children
                    .get_mut(&link.sink_child)
                    .ok_or_else(|| KernelCompositeError::StaleChild(link.sink_child.clone()))?
                    .remote_admit(link.sink_endpoint, link.sink_cord, sequence, &link.transfer)
                    .map_err(|reason| KernelCompositeError::InternalTransport {
                        link: link_index,
                        reason,
                    })?;
                if matches!(accepted, RemoteIngressOutcome::Accepted { .. }) {
                    children
                        .get_mut(&link.source_child)
                        .ok_or_else(|| KernelCompositeError::StaleChild(link.source_child.clone()))?
                        .remote_delivered(link.source_endpoint, link.source_cord, sequence)
                        .map_err(|reason| KernelCompositeError::InternalTransport {
                            link: link_index,
                            reason,
                        })?;
                }
            } else {
                let terminal = children
                    .get(&link.source_child)
                    .ok_or_else(|| KernelCompositeError::StaleChild(link.source_child.clone()))?
                    .remote_terminal_disposition(link.source_endpoint, link.source_cord)
                    .map_err(|reason| KernelCompositeError::InternalTransport {
                        link: link_index,
                        reason,
                    })?;
                match terminal {
                    Some(RemoteTerminalDisposition::NormalClose) => {
                        children
                            .get_mut(&link.sink_child)
                            .ok_or_else(|| {
                                KernelCompositeError::StaleChild(link.sink_child.clone())
                            })?
                            .remote_close(link.sink_endpoint, link.sink_cord)
                            .map_err(|reason| KernelCompositeError::InternalTransport {
                                link: link_index,
                                reason,
                            })?;
                        link.closed = true;
                    }
                    Some(RemoteTerminalDisposition::Abnormal) => {
                        let abnormal = children
                            .get(&link.source_child)
                            .ok_or_else(|| {
                                KernelCompositeError::StaleChild(link.source_child.clone())
                            })?
                            .remote_abnormal_terminal(link.source_endpoint, link.source_cord)
                            .map_err(|reason| KernelCompositeError::InternalTransport {
                                link: link_index,
                                reason,
                            })?
                            .ok_or(KernelCompositeError::InternalTransport {
                                link: link_index,
                                reason: ChildTransportError::TransferBufferTooSmall,
                            })?;
                        children
                            .get_mut(&link.sink_child)
                            .ok_or_else(|| {
                                KernelCompositeError::StaleChild(link.sink_child.clone())
                            })?
                            .remote_close_abnormal(link.sink_endpoint, link.sink_cord, abnormal)
                            .map_err(|reason| KernelCompositeError::InternalTransport {
                                link: link_index,
                                reason,
                            })?;
                        link.closed = true;
                    }
                    None => {}
                }
            }
        }
        Ok(())
    }
}

fn dispatch_matches(
    exact: &KernelCompositeHostCallObligation,
    host: &PreparationHostIdentity,
    resources: &[ResourceBinding],
    authorities: &[AuthorityBinding],
) -> bool {
    &exact.host == host && exact.resources == resources && exact.authorities == authorities
}

fn invalid_host_call_token() -> KernelCompositeError {
    KernelCompositeError::InvalidHostCallToken
}

fn outstanding_host_call_index(
    outstanding: &[Option<OutstandingHostCall>],
    token: u64,
) -> Result<usize, KernelCompositeError> {
    outstanding
        .iter()
        .position(|slot| slot.as_ref().is_some_and(|item| item.token == token))
        .ok_or_else(invalid_host_call_token)
}

fn host_call_obligation_identity(
    plan: &str,
    fragment: &str,
    placement: &str,
    call: HostCallId,
    contract: &str,
) -> [u8; 32] {
    let mut exact =
        Vec::with_capacity(plan.len() + fragment.len() + placement.len() + contract.len() + 5);
    exact.extend_from_slice(plan.as_bytes());
    exact.push(0);
    exact.extend_from_slice(fragment.as_bytes());
    exact.push(0);
    exact.extend_from_slice(placement.as_bytes());
    exact.push(0);
    exact.extend_from_slice(&call.0.to_le_bytes());
    exact.extend_from_slice(contract.as_bytes());
    semantic_digest("conduit/planned-activation-host-call-obligation@1", &exact)
}

fn internal_links(
    preparation: &KernelCompositePreparation,
) -> Result<Vec<InternalLink>, KernelCompositeError> {
    type Endpoint = (HostId, RemoteEndpointId, conduit_kernel::CordId, usize);
    let mut rows = BTreeMap::<ConnectionId, (Option<Endpoint>, Option<Endpoint>)>::new();
    for (child, lowered) in preparation.children() {
        for endpoint in &lowered.remote_endpoints {
            let row = rows
                .entry(endpoint.connection_id.clone())
                .or_insert((None, None));
            let maximum = lowered
                .cords
                .iter()
                .find(|cord| cord.spec.cord == endpoint.cord)
                .map(|cord| cord.spec.maximum_value_bytes as usize)
                .ok_or_else(|| {
                    KernelCompositeError::InvalidBoundary("remote endpoint Cord is absent".into())
                })?;
            let value = (child.clone(), endpoint.endpoint, endpoint.cord, maximum);
            match endpoint.direction {
                RemoteCordDirection::Egress => row.0 = Some(value),
                RemoteCordDirection::Ingress => row.1 = Some(value),
            }
        }
    }
    rows.into_iter()
        .map(|(connection_id, (source, sink))| {
            let (source_child, source_endpoint, source_cord, maximum_value_bytes) = source
                .ok_or_else(|| {
                    KernelCompositeError::InvalidBoundary(format!(
                        "internal Cord '{}' has no source child",
                        connection_id.as_str()
                    ))
                })?;
            let (sink_child, sink_endpoint, sink_cord, _) = sink.ok_or_else(|| {
                KernelCompositeError::InvalidBoundary(format!(
                    "internal Cord '{}' has no sink child",
                    connection_id.as_str()
                ))
            })?;
            Ok(InternalLink {
                connection_id,
                source_child,
                source_endpoint,
                source_cord,
                sink_child,
                sink_endpoint,
                sink_cord,
                closed: false,
                transfer: Vec::with_capacity(maximum_value_bytes),
            })
        })
        .collect()
}

fn invalid_front(port_id: &PortId, reason: &str) -> KernelCompositeError {
    KernelCompositeError::InvalidBoundary(format!("front '{}': {reason}", port_id.as_str()))
}

fn execution(child: &HostId, reason: String) -> KernelCompositeError {
    KernelCompositeError::Execution {
        child: child.clone(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{seal_plan, PlotIdentity};

    #[test]
    fn empty_composite_is_refused_before_any_child_is_admitted() {
        let plan = seal_plan(
            PlotIdentity {
                source_document_id: "source".into(),
                checked_plot_id: "checked".into(),
                expanded_plot_id: "expanded".into(),
            },
            vec![],
        );
        assert_eq!(
            KernelCompositePreparation::prepare(plan),
            Err(KernelCompositeError::Empty)
        );
    }

    #[test]
    fn host_call_obligation_commits_every_routing_identity() {
        let exact = host_call_obligation_identity(
            "plan",
            "fragment",
            "placement",
            HostCallId(0),
            "contract",
        );
        for drifted in [
            host_call_obligation_identity(
                "other",
                "fragment",
                "placement",
                HostCallId(0),
                "contract",
            ),
            host_call_obligation_identity("plan", "other", "placement", HostCallId(0), "contract"),
            host_call_obligation_identity("plan", "fragment", "other", HostCallId(0), "contract"),
            host_call_obligation_identity(
                "plan",
                "fragment",
                "placement",
                HostCallId(1),
                "contract",
            ),
            host_call_obligation_identity("plan", "fragment", "placement", HostCallId(0), "other"),
        ] {
            assert_ne!(drifted, exact);
        }
    }

    #[test]
    fn dispatch_token_refuses_unknown_stale_and_replayed_tokens_without_aliasing_live_tokens() {
        use conduit_kernel::{BoundedValueRef, RequestId, ValueRef};
        let request = HostCallRequest {
            node: NodeId(1),
            request: RequestId(2),
            call: HostCallId(3),
            input: BoundedValueRef::new(
                ValueRef {
                    slot: 4,
                    generation: 5,
                    byte_len: 6,
                },
                6,
            )
            .unwrap(),
        };
        let identity = [7; 32];
        let mut outstanding = vec![
            Some(OutstandingHostCall {
                token: 9,
                child_index: 0,
                request,
                obligation_identity: identity,
            }),
            Some(OutstandingHostCall {
                token: 10,
                child_index: 1,
                request: HostCallRequest {
                    request: RequestId(8),
                    ..request
                },
                obligation_identity: [8; 32],
            }),
        ];
        let exact = KernelCompositeHostRequest { dispatch_token: 9 };
        let other_live = KernelCompositeHostRequest { dispatch_token: 10 };
        assert_eq!(
            outstanding_host_call_index(&outstanding, exact.dispatch_token),
            Ok(0)
        );
        assert_eq!(
            outstanding_host_call_index(&outstanding, other_live.dispatch_token),
            Ok(1)
        );
        assert!(outstanding_host_call_index(&outstanding, 11).is_err());
        outstanding[0] = None;
        assert!(outstanding_host_call_index(&outstanding, exact.dispatch_token).is_err());
        assert!(outstanding_host_call_index(&outstanding, exact.dispatch_token).is_err());
        assert_eq!(
            outstanding_host_call_index(&outstanding, other_live.dispatch_token),
            Ok(1)
        );
    }

    #[test]
    fn dispatch_requires_exact_current_host_resources_and_authority() {
        use conduit_core::{
            AuthorityContractId, AuthorityGrantId, BootId, CapabilityId, HostCallContractId,
            KindId, OfferGeneration, ResourceClassId, ResourcePoolId,
        };
        let host = PreparationHostIdentity {
            host_id: HostId::from("host"),
            boot_id: BootId::from("boot"),
            offer_generation: OfferGeneration(1),
        };
        let resource = ResourceBinding {
            pool_id: ResourcePoolId::from("pool"),
            class_id: ResourceClassId::from("class"),
            units: 1,
            protected: None,
            compute: None,
            content: None,
        };
        let authority = AuthorityBinding {
            grant_id: AuthorityGrantId::from("grant"),
            contract_id: AuthorityContractId::from("authority"),
            host_call_contract_id: HostCallContractId::from("call"),
            subject_kind: KindId::from("subject"),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            capability_id: CapabilityId::from("capability"),
        };
        let exact = KernelCompositeHostCallObligation {
            host: host.clone(),
            requirement: HostCallRequirement {
                contract_id: HostCallContractId::from("call"),
                target_kind: Some(KindId::from("subject")),
                maximum_in_flight: 1,
                maximum_input_bytes: 1,
                maximum_output_bytes: 1,
            },
            resources: vec![resource.clone()],
            authorities: vec![authority.clone()],
        };
        assert!(dispatch_matches(
            &exact,
            &host,
            core::slice::from_ref(&resource),
            core::slice::from_ref(&authority)
        ));
        assert!(!dispatch_matches(
            &exact,
            &host,
            &[],
            core::slice::from_ref(&authority)
        ));
        assert!(!dispatch_matches(
            &exact,
            &host,
            core::slice::from_ref(&resource),
            &[]
        ));
        let mut stale = host;
        stale.offer_generation = OfferGeneration(2);
        assert!(!dispatch_matches(
            &exact,
            &stale,
            core::slice::from_ref(&resource),
            core::slice::from_ref(&authority)
        ));
    }

    #[test]
    fn fixed_dispatch_slots_reuse_without_capacity_growth() {
        let mut slots: Vec<Option<OutstandingHostCall>> = (0..2).map(|_| None).collect();
        let capacity = slots.capacity();
        for token in 0..32 {
            slots[0] = Some(OutstandingHostCall {
                token,
                child_index: 0,
                request: HostCallRequest {
                    node: NodeId(0),
                    request: conduit_kernel::RequestId(token as u32),
                    call: HostCallId(0),
                    input: conduit_kernel::BoundedValueRef::new(
                        conduit_kernel::ValueRef {
                            slot: 0,
                            generation: 0,
                            byte_len: 0,
                        },
                        0,
                    )
                    .unwrap(),
                },
                obligation_identity: [0; 32],
            });
            slots[0] = None;
        }
        assert_eq!(slots.capacity(), capacity);
        assert_eq!(slots.len(), 2);
    }
}
