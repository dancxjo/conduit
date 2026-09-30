use crate::child::{BoundaryEndpoint, BoundaryTerminal, ChildKernel};
use crate::{KernelCompositeDefinition, KernelOperationRegistry};
use conduit_core::{
    bind_active_play, semantic_digest, ActivePlayId, ConnectionId, HostId, Plan, PortDirection,
    PortId, ValuePayload,
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
    Lowering { child: HostId, error: LoweringError },
    InvalidBoundary(String),
    ChildRefused { child: HostId, reason: String },
    Execution { child: HostId, reason: String },
    UnknownFront(PortId),
    StaleChild(HostId),
    MalformedBoundary(PortId),
    InvalidLifecycle,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelCompositeHostRequest {
    pub dispatch_token: u64,
    pub child: HostId,
    pub request: HostCallRequest,
    /// Commitment to the selected fragment, placement, call and Host Call
    /// contract. Completion must return this exact sealed obligation identity.
    pub obligation_identity: [u8; 32],
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
    Abnormal(ValuePayload),
}

pub struct KernelCompositeHost {
    definition: KernelCompositeDefinition,
    children: BTreeMap<HostId, ChildKernel>,
    fronts: BTreeMap<PortId, FaceRoute>,
    links: Vec<InternalLink>,
    active_plays: BTreeMap<HostId, ActivePlayId>,
    started: bool,
    cancelled: bool,
    host_call_obligations: BTreeMap<(HostId, NodeId, HostCallId), [u8; 32]>,
    outstanding_host_calls: BTreeMap<u64, (HostId, HostCallRequest, [u8; 32])>,
    next_dispatch_token: u64,
}

impl KernelCompositeHost {
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
                let placement = lowered.identity.placement_for_node(*node).ok_or_else(|| {
                    KernelCompositeError::InvalidBoundary("Host Call owner is absent".into())
                })?;
                host_call_obligations.insert(
                    (child.clone(), *node, *call),
                    host_call_obligation_identity(
                        lowered.identity.plan_id.as_str(),
                        lowered.identity.fragment_id.as_str(),
                        placement.as_str(),
                        *call,
                        contract.as_str(),
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
        Ok(Self {
            definition,
            children,
            fronts,
            links,
            active_plays: BTreeMap::new(),
            started: false,
            cancelled: false,
            host_call_obligations,
            outstanding_host_calls: BTreeMap::new(),
            next_dispatch_token: 0,
        })
    }

    pub fn definition(&self) -> &KernelCompositeDefinition {
        &self.definition
    }

    pub fn start(&mut self) -> Result<&BTreeMap<HostId, ActivePlayId>, KernelCompositeError> {
        if self.started || self.cancelled {
            return Err(KernelCompositeError::InvalidLifecycle);
        }
        self.active_plays = self
            .definition
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
        let route = self.front(port_id, PortDirection::Input)?.clone();
        self.children
            .get_mut(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .admit_boundary(port_id, sequence, value)
            .map_err(|_| KernelCompositeError::MalformedBoundary(port_id.clone()))
    }

    pub fn close_input(&mut self, port_id: &PortId) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let route = self.front(port_id, PortDirection::Input)?.clone();
        self.children
            .get_mut(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .close_boundary(port_id)
            .map_err(|reason| execution(&route.child, reason))
    }

    pub fn close_input_abnormal(
        &mut self,
        port_id: &PortId,
        terminal: &ValuePayload,
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let route = self.front(port_id, PortDirection::Input)?.clone();
        self.children
            .get_mut(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .close_boundary_abnormal(port_id, terminal)
            .map_err(|reason| execution(&route.child, reason))
    }

    pub fn output(
        &mut self,
        port_id: &PortId,
    ) -> Result<Option<(u64, ValuePayload)>, KernelCompositeError> {
        self.require_started()?;
        let route = self.front(port_id, PortDirection::Output)?.clone();
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
        let route = self.front(port_id, PortDirection::Output)?.clone();
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
        let route = self.front(port_id, PortDirection::Output)?.clone();
        self.children
            .get_mut(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .deliver_boundary(port_id, sequence)
            .map_err(|reason| execution(&route.child, reason))
    }

    pub fn output_terminal(
        &self,
        port_id: &PortId,
    ) -> Result<Option<KernelCompositeTerminal>, KernelCompositeError> {
        self.require_started()?;
        let route = self.front(port_id, PortDirection::Output)?.clone();
        self.children
            .get(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .boundary_terminal(port_id)
            .map(|terminal| {
                terminal.map(|terminal| match terminal {
                    BoundaryTerminal::Normal => KernelCompositeTerminal::Normal,
                    BoundaryTerminal::Abnormal(value) => KernelCompositeTerminal::Abnormal(value),
                })
            })
            .map_err(|reason| execution(&route.child, reason))
    }

    pub fn next_host_request(&mut self) -> Option<KernelCompositeHostRequest> {
        if !self.started || self.cancelled {
            return None;
        }
        let next_dispatch_token = self.next_dispatch_token.checked_add(1)?;
        let surfaced = self.children.iter_mut().find_map(|(child, kernel)| {
            kernel
                .next_host_request()
                .map(|request| (child.clone(), request))
        })?;
        let (child, request) = surfaced;
        let obligation_identity = self
            .host_call_obligations
            .get(&(child.clone(), request.node, request.call))
            .copied()
            .expect("lowered Host Call has a sealed obligation");
        let dispatch_token = self.next_dispatch_token;
        self.next_dispatch_token = next_dispatch_token;
        if self
            .outstanding_host_calls
            .insert(
                dispatch_token,
                (child.clone(), request, obligation_identity),
            )
            .is_some()
        {
            return None;
        }
        Some(KernelCompositeHostRequest {
            dispatch_token,
            child,
            request,
            obligation_identity,
        })
    }

    pub fn complete_host_call(
        &mut self,
        request: &KernelCompositeHostRequest,
        outcome: HostCallOutcome,
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        self.consume_host_call_obligation(request)?;
        self.children
            .get_mut(&request.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(request.child.clone()))?
            .complete_host_call(request.request.node, request.request.request, outcome)
            .map_err(|reason| execution(&request.child, reason))
    }

    /// Resolve the exact admitted input for a surfaced Host Call.
    pub fn host_request_input(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<&[u8], KernelCompositeError> {
        self.verify_host_call_obligation(request)?;
        self.children
            .get(&request.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(request.child.clone()))?
            .host_value(request.request.input.value)
            .map_err(|reason| execution(&request.child, reason))
    }

    /// Store a bounded adapter result in the owning child and complete its call.
    pub fn complete_host_call_bytes(
        &mut self,
        request: &KernelCompositeHostRequest,
        bytes: &[u8],
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        self.consume_host_call_obligation(request)?;
        let child = self
            .children
            .get_mut(&request.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(request.child.clone()))?;
        let value = child
            .store_host_value(bytes)
            .map_err(|reason| execution(&request.child, reason))?;
        let output = conduit_kernel::BoundedValueRef::new(value, bytes.len() as u32)
            .map_err(|error| execution(&request.child, format!("{error:?}")))?;
        child
            .complete_host_call(
                request.request.node,
                request.request.request,
                conduit_kernel::HostCallOutcome {
                    disposition: conduit_kernel::HostCallDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            )
            .map_err(|reason| execution(&request.child, reason))
    }

    fn verify_host_call_obligation(
        &self,
        request: &KernelCompositeHostRequest,
    ) -> Result<(), KernelCompositeError> {
        verify_outstanding_host_call(&self.outstanding_host_calls, request)
    }

    fn consume_host_call_obligation(
        &mut self,
        request: &KernelCompositeHostRequest,
    ) -> Result<(), KernelCompositeError> {
        self.verify_host_call_obligation(request)?;
        self.outstanding_host_calls.remove(&request.dispatch_token);
        Ok(())
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
        self.outstanding_host_calls.clear();
        for (child, kernel) in &mut self.children {
            kernel.cancel().map_err(|reason| execution(child, reason))?;
        }
        self.cancelled = true;
        Ok(())
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
            .ok_or_else(|| KernelCompositeError::UnknownFront(port_id.clone()))
    }

    fn require_started(&self) -> Result<(), KernelCompositeError> {
        if self.started && !self.cancelled {
            Ok(())
        } else {
            Err(KernelCompositeError::InvalidLifecycle)
        }
    }

    fn pump_internal(&mut self) -> Result<(), KernelCompositeError> {
        for index in 0..self.links.len() {
            let link = self.links[index].clone();
            if link.closed {
                continue;
            }
            let offer = self
                .children
                .get_mut(&link.source_child)
                .ok_or_else(|| KernelCompositeError::StaleChild(link.source_child.clone()))?
                .remote_offer(link.source_endpoint, link.source_cord)
                .map_err(|reason| execution(&link.source_child, reason))?;
            if let Some((sequence, bytes)) = offer {
                let accepted = self
                    .children
                    .get_mut(&link.sink_child)
                    .ok_or_else(|| KernelCompositeError::StaleChild(link.sink_child.clone()))?
                    .remote_admit(link.sink_endpoint, link.sink_cord, sequence, &bytes)
                    .map_err(|reason| execution(&link.sink_child, reason))?;
                if matches!(accepted, RemoteIngressOutcome::Accepted { .. }) {
                    self.children
                        .get_mut(&link.source_child)
                        .ok_or_else(|| KernelCompositeError::StaleChild(link.source_child.clone()))?
                        .remote_delivered(link.source_endpoint, link.source_cord, sequence)
                        .map_err(|reason| execution(&link.source_child, reason))?;
                }
            } else {
                let terminal = self
                    .children
                    .get(&link.source_child)
                    .ok_or_else(|| KernelCompositeError::StaleChild(link.source_child.clone()))?
                    .remote_terminal_disposition(link.source_endpoint, link.source_cord)
                    .map_err(|reason| execution(&link.source_child, reason))?;
                match terminal {
                    Some(RemoteTerminalDisposition::NormalClose) => {
                        self.children
                            .get_mut(&link.sink_child)
                            .ok_or_else(|| {
                                KernelCompositeError::StaleChild(link.sink_child.clone())
                            })?
                            .remote_close(link.sink_endpoint, link.sink_cord)
                            .map_err(|reason| execution(&link.sink_child, reason))?;
                        self.links[index].closed = true;
                    }
                    Some(RemoteTerminalDisposition::Abnormal) => {
                        let abnormal = self
                            .children
                            .get(&link.source_child)
                            .ok_or_else(|| {
                                KernelCompositeError::StaleChild(link.source_child.clone())
                            })?
                            .remote_abnormal_terminal(link.source_endpoint, link.source_cord)
                            .map_err(|reason| execution(&link.source_child, reason))?
                            .ok_or_else(|| {
                                execution(
                                    &link.source_child,
                                    "abnormal internal terminal omitted its exact value".into(),
                                )
                            })?;
                        self.children
                            .get_mut(&link.sink_child)
                            .ok_or_else(|| {
                                KernelCompositeError::StaleChild(link.sink_child.clone())
                            })?
                            .remote_close_abnormal(link.sink_endpoint, link.sink_cord, abnormal)
                            .map_err(|reason| execution(&link.sink_child, reason))?;
                        self.links[index].closed = true;
                    }
                    None => {}
                }
            }
        }
        Ok(())
    }
}

fn verify_outstanding_host_call(
    outstanding: &BTreeMap<u64, (HostId, HostCallRequest, [u8; 32])>,
    request: &KernelCompositeHostRequest,
) -> Result<(), KernelCompositeError> {
    let expected = outstanding.get(&request.dispatch_token);
    if expected
        == Some(&(
            request.child.clone(),
            request.request,
            request.obligation_identity,
        ))
    {
        Ok(())
    } else {
        Err(KernelCompositeError::InvalidBoundary(
            "Host Call completion differs from its outstanding sealed dispatch".into(),
        ))
    }
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
    type Endpoint = (HostId, RemoteEndpointId, conduit_kernel::CordId);
    let mut rows = BTreeMap::<ConnectionId, (Option<Endpoint>, Option<Endpoint>)>::new();
    for (child, lowered) in preparation.children() {
        for endpoint in &lowered.remote_endpoints {
            let row = rows
                .entry(endpoint.connection_id.clone())
                .or_insert((None, None));
            let value = (child.clone(), endpoint.endpoint, endpoint.cord);
            match endpoint.direction {
                RemoteCordDirection::Egress => row.0 = Some(value),
                RemoteCordDirection::Ingress => row.1 = Some(value),
            }
        }
    }
    rows.into_iter()
        .map(|(connection_id, (source, sink))| {
            let (source_child, source_endpoint, source_cord) = source.ok_or_else(|| {
                KernelCompositeError::InvalidBoundary(format!(
                    "internal Cord '{}' has no source child",
                    connection_id.as_str()
                ))
            })?;
            let (sink_child, sink_endpoint, sink_cord) = sink.ok_or_else(|| {
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
    use conduit_core::{seal_plan, FormIdentity};

    #[test]
    fn empty_composite_is_refused_before_any_child_is_admitted() {
        let plan = seal_plan(
            FormIdentity {
                source_document_id: "source".into(),
                checked_form_id: "checked".into(),
                expanded_form_id: "expanded".into(),
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
    fn dispatch_token_refuses_forged_cross_swapped_duplicate_and_late_completion() {
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
        let child = HostId::from("child");
        let identity = [7; 32];
        let mut outstanding = BTreeMap::from([(9, (child.clone(), request, identity))]);
        let exact = KernelCompositeHostRequest {
            dispatch_token: 9,
            child,
            request,
            obligation_identity: identity,
        };
        assert!(verify_outstanding_host_call(&outstanding, &exact).is_ok());
        let mut forged = exact.clone();
        forged.request.request = RequestId(8);
        assert!(verify_outstanding_host_call(&outstanding, &forged).is_err());
        let mut swapped = exact.clone();
        swapped.request.input.value.generation = 8;
        assert!(verify_outstanding_host_call(&outstanding, &swapped).is_err());
        outstanding.remove(&exact.dispatch_token);
        assert!(verify_outstanding_host_call(&outstanding, &exact).is_err());
        assert!(verify_outstanding_host_call(&outstanding, &exact).is_err());
    }
}
