use crate::child::{BoundaryEndpoint, ChildKernel};
use crate::{KernelCompositeDefinition, KernelOperationRegistry};
use conduit_core::{
    bind_active_play, ActivePlayId, ConnectionId, HostId, Plan, PortDirection, PortId, ValuePayload,
};
use conduit_kernel::scheduler::{HostOperationRequest, RemoteIngressOutcome, SchedulerStatus};
use conduit_kernel::{HostOperationOutcome, KernelEvent, RemoteEndpointId};
use conduit_runtime::lowering::{
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
    UnknownFace(PortId),
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
    pub child: HostId,
    pub request: HostOperationRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelCompositeStatus {
    Active,
    Complete,
    Cancelled,
}

pub struct KernelCompositeHost {
    definition: KernelCompositeDefinition,
    children: BTreeMap<HostId, ChildKernel>,
    faces: BTreeMap<PortId, FaceRoute>,
    links: Vec<InternalLink>,
    active_plays: BTreeMap<HostId, ActivePlayId>,
    started: bool,
    cancelled: bool,
}

impl KernelCompositeHost {
    pub fn prepare(
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, KernelCompositeError> {
        let preparation = KernelCompositePreparation::prepare(definition.internal_plan.clone())?;
        let mut child_boundaries = BTreeMap::<HostId, Vec<BoundaryEndpoint>>::new();
        let mut faces = BTreeMap::new();
        for face in definition
            .boundary
            .input_faces
            .iter()
            .chain(&definition.boundary.output_faces)
        {
            if faces.contains_key(&face.external_port.port_id) {
                return Err(KernelCompositeError::InvalidBoundary(format!(
                    "duplicate external face '{}'",
                    face.external_port.port_id.as_str()
                )));
            }
            let lowered = preparation
                .child(&face.internal_child)
                .ok_or_else(|| KernelCompositeError::StaleChild(face.internal_child.clone()))?;
            let node = lowered
                .identity
                .node_for_placement(&face.internal_placement_id)
                .ok_or_else(|| invalid_face(&face.external_port.port_id, "missing placement"))?;
            lowered
                .identity
                .port_for_identity(node, face.external_port.direction, &face.internal_port_id)
                .ok_or_else(|| {
                    invalid_face(&face.external_port.port_id, "missing internal port")
                })?;
            let boundary_count = child_boundaries
                .get(&face.internal_child)
                .map_or(0, Vec::len);
            let endpoint = RemoteEndpointId(
                u16::try_from(lowered.remote_endpoints.len() + boundary_count)
                    .map_err(|_| invalid_face(&face.external_port.port_id, "endpoint overflow"))?,
            );
            let cord = conduit_kernel::CordId(
                u16::try_from(lowered.cords.len() + boundary_count)
                    .map_err(|_| invalid_face(&face.external_port.port_id, "Cord overflow"))?,
            );
            child_boundaries
                .entry(face.internal_child.clone())
                .or_default()
                .push(BoundaryEndpoint {
                    external_port_id: face.external_port.port_id.clone(),
                    internal_port_id: face.internal_port_id.clone(),
                    endpoint,
                    cord,
                    direction: face.external_port.direction,
                    value_kind: face.external_port.value_kind.clone(),
                    item_capacity: definition.external_capability.limits.max_queue_items,
                    byte_capacity: definition.external_capability.limits.max_queue_bytes,
                });
            faces.insert(
                face.external_port.port_id.clone(),
                FaceRoute {
                    child: face.internal_child.clone(),
                    direction: face.external_port.direction,
                },
            );
        }

        let links = internal_links(&preparation)?;
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
            faces,
            links,
            active_plays: BTreeMap::new(),
            started: false,
            cancelled: false,
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
        let route = self.face(port_id, PortDirection::Input)?.clone();
        self.children
            .get_mut(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .admit_boundary(port_id, sequence, value)
            .map_err(|_| KernelCompositeError::MalformedBoundary(port_id.clone()))
    }

    pub fn close_input(&mut self, port_id: &PortId) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let route = self.face(port_id, PortDirection::Input)?.clone();
        self.children
            .get_mut(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .close_boundary(port_id)
            .map_err(|reason| execution(&route.child, reason))
    }

    pub fn output(
        &mut self,
        port_id: &PortId,
    ) -> Result<Option<(u64, ValuePayload)>, KernelCompositeError> {
        self.require_started()?;
        let route = self.face(port_id, PortDirection::Output)?.clone();
        self.children
            .get_mut(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .boundary_output(port_id)
            .map_err(|reason| execution(&route.child, reason))
    }

    pub fn complete_output(
        &mut self,
        port_id: &PortId,
        sequence: u64,
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        let route = self.face(port_id, PortDirection::Output)?.clone();
        self.children
            .get_mut(&route.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(route.child.clone()))?
            .deliver_boundary(port_id, sequence)
            .map_err(|reason| execution(&route.child, reason))
    }

    pub fn next_host_request(&mut self) -> Option<KernelCompositeHostRequest> {
        if !self.started || self.cancelled {
            return None;
        }
        self.children.iter_mut().find_map(|(child, kernel)| {
            kernel
                .next_host_request()
                .map(|request| KernelCompositeHostRequest {
                    child: child.clone(),
                    request,
                })
        })
    }

    pub fn complete_host_operation(
        &mut self,
        request: &KernelCompositeHostRequest,
        outcome: HostOperationOutcome,
    ) -> Result<(), KernelCompositeError> {
        self.require_started()?;
        self.children
            .get_mut(&request.child)
            .ok_or_else(|| KernelCompositeError::StaleChild(request.child.clone()))?
            .complete_host_operation(request.request.node, request.request.request, outcome)
            .map_err(|reason| execution(&request.child, reason))
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
            .all(|child| child.status() == SchedulerStatus::Complete)
            && self.links.iter().all(|link| link.closed)
        {
            Ok(KernelCompositeStatus::Complete)
        } else {
            Ok(KernelCompositeStatus::Active)
        }
    }

    pub fn cancel(&mut self) -> Result<(), KernelCompositeError> {
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

    fn face(
        &self,
        port_id: &PortId,
        direction: PortDirection,
    ) -> Result<&FaceRoute, KernelCompositeError> {
        self.faces
            .get(port_id)
            .filter(|route| route.direction == direction)
            .ok_or_else(|| KernelCompositeError::UnknownFace(port_id.clone()))
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
                    .remote_terminal(link.source_endpoint, link.source_cord)
                    .map_err(|reason| execution(&link.source_child, reason))?;
                if terminal {
                    self.children
                        .get_mut(&link.sink_child)
                        .ok_or_else(|| KernelCompositeError::StaleChild(link.sink_child.clone()))?
                        .remote_close(link.sink_endpoint, link.sink_cord)
                        .map_err(|reason| execution(&link.sink_child, reason))?;
                    self.links[index].closed = true;
                }
            }
        }
        Ok(())
    }
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

fn invalid_face(port_id: &PortId, reason: &str) -> KernelCompositeError {
    KernelCompositeError::InvalidBoundary(format!("face '{}': {reason}", port_id.as_str()))
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
}
