//! Lower exact plans and admit child kernels, boundary routes and Host Call obligations.
use super::*;
use conduit_core::{bind_active_play, semantic_digest};
use conduit_plan_lowering::lowering::{lower_plan_fragment, RemoteCordDirection};

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

impl KernelCompositeHost {
    pub fn prepare(
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, KernelCompositeError> {
        Self::prepare_with_sign_storage(definition, registry, KernelCompositeSignStorage::default())
    }

    /// Admit explicit extra finite Sign storage before any child starts Play.
    pub fn prepare_with_sign_storage(
        definition: KernelCompositeDefinition,
        registry: &KernelOperationRegistry,
        sign_storage: KernelCompositeSignStorage,
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
                    child_index: preparation
                        .children
                        .keys()
                        .position(|child| child == &front.internal_child)
                        .expect("validated child remains in the prepared plan"),
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
                sign_storage,
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
            parent_play_binding: None,
            started: false,
            cancelled: false,
            host_call_obligations,
            outstanding_host_calls: (0..outstanding_host_call_bound).map(|_| None).collect(),
            next_dispatch_token: 0,
            cancellation_failures: Vec::with_capacity(child_count),
        })
    }
}

fn internal_links(
    preparation: &KernelCompositePreparation,
) -> Result<Vec<InternalLink>, KernelCompositeError> {
    type Endpoint = (
        HostId,
        usize,
        RemoteEndpointId,
        conduit_kernel::CordId,
        usize,
    );
    let mut rows = BTreeMap::<ConnectionId, (Option<Endpoint>, Option<Endpoint>)>::new();
    for (child_index, (child, lowered)) in preparation.children().enumerate() {
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
            let value = (
                child.clone(),
                child_index,
                endpoint.endpoint,
                endpoint.cord,
                maximum,
            );
            match endpoint.direction {
                RemoteCordDirection::Egress => row.0 = Some(value),
                RemoteCordDirection::Ingress => row.1 = Some(value),
            }
        }
    }
    rows.into_iter()
        .map(|(connection_id, (source, sink))| {
            let (source_child, source_index, source_endpoint, source_cord, maximum_value_bytes) =
                source.ok_or_else(|| {
                    KernelCompositeError::InvalidBoundary(format!(
                        "internal Cord '{}' has no source child",
                        connection_id.as_str()
                    ))
                })?;
            let (sink_child, sink_index, sink_endpoint, sink_cord, _) = sink.ok_or_else(|| {
                KernelCompositeError::InvalidBoundary(format!(
                    "internal Cord '{}' has no sink child",
                    connection_id.as_str()
                ))
            })?;
            Ok(InternalLink {
                connection_id,
                source_child,
                source_index,
                source_endpoint,
                source_cord,
                sink_child,
                sink_index,
                sink_endpoint,
                sink_cord,
                closed: false,
                transfer: Vec::with_capacity(maximum_value_bytes),
            })
        })
        .collect()
}

pub(super) fn host_call_obligation_identity(
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
