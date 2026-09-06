//! Admission and finite storage preparation for the browser production kernel.

use super::*;

pub(super) fn validate_envelope(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    allow_one_remote_endpoint: bool,
) -> Result<(), String> {
    let route_targets = lowered
        .routes
        .iter()
        .map(|route| route.targets.len())
        .sum::<usize>();
    if lowered.nodes.is_empty()
        || lowered.nodes.len() > MAXIMUM_BROWSER_GEARS
        || lowered.cords.is_empty()
        || lowered.cords.len() > MAXIMUM_BROWSER_CORDS
        || lowered.cord_value_slots as usize > BROWSER_QUEUE_SLOTS
        || lowered.routes.len() > BROWSER_ROUTE_SLOTS
        || route_targets > BROWSER_ROUTE_TARGETS
        || if allow_one_remote_endpoint {
            lowered.remote_endpoints.len() != 1
        } else {
            !lowered.remote_endpoints.is_empty()
        }
        || lowered.host_operations.len() > BROWSER_HOST_OPERATION_BINDINGS
        || fragment
            .placements
            .iter()
            .any(|placement| factory(&placement.implementation_id).is_none())
    {
        return Err("Form exceeds the installed finite browser execution envelope".into());
    }
    Ok(())
}

pub(super) fn prepare_scheduler(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<TourScheduler, String> {
    prepare_partition_scheduler(&[(fragment, lowered)])
}

/// Compose already-lowered exact partitions without synthesizing a Plan.
pub(super) fn prepare_partition_scheduler(
    partitions: &[(&PlanFragment, &LoweredPlanFragment)],
) -> Result<TourScheduler, String> {
    if partitions.is_empty()
        || partitions.len() > conduit_body::MAX_BODY_FORMS
        || partitions.iter().any(|(fragment, part)| {
            part.identity.plan_id != fragment.plan_id
                || part.identity.fragment_id != fragment.fragment_id
                || part.nodes.len() != fragment.placements.len()
        })
    {
        return Err("browser partitions do not match their original Plans".into());
    }
    let active_nodes = partitions
        .iter()
        .map(|(_, part)| part.nodes.len())
        .sum::<usize>();
    let active_cords = partitions
        .iter()
        .map(|(_, part)| part.cords.len())
        .sum::<usize>();
    if active_nodes > MAXIMUM_BROWSER_GEARS || active_cords > MAXIMUM_BROWSER_CORDS {
        return Err("Body exceeds the installed browser kernel tables".into());
    }
    let mut values = HostedValueStore::new(
        BROWSER_VALUE_ITEMS,
        MAXIMUM_BROWSER_VALUE_BYTES as u32,
        BROWSER_TOTAL_VALUE_BYTES,
    )
    .map_err(|error| format!("browser value store: {error:?}"))?;
    let mut operations = Vec::with_capacity(MAXIMUM_BROWSER_GEARS);
    let mut mappings = [None; MAXIMUM_BROWSER_GEARS];
    let mut snapshots = core::array::from_fn(|_| None);
    let mut selectors = core::array::from_fn(|_| None);
    let mut attempts = core::array::from_fn(|_| None);
    let mut comparisons = core::array::from_fn(|_| None);
    let mut timing = core::array::from_fn(|_| None);
    for (fragment, node) in partitions
        .iter()
        .flat_map(|(fragment, part)| part.nodes.iter().map(move |node| (*fragment, node)))
    {
        if usize::from(node.node.0) != operations.len() {
            return Err("browser partition nodes are not contiguous".into());
        }
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.placement_id == node.placement_id)
            .ok_or_else(|| "lowered browser node has no planned placement".to_string())?;
        let installation = factory(&placement.implementation_id)
            .ok_or_else(|| "planned browser implementation is not installed".to_string())?;
        if placement.kind_id.as_str() == conduit_semantic_catalog::QUANTITY_MAP_KIND {
            mappings[usize::from(node.node.0)] = Some(
                crate::installed_browser::prepare_quantity_mapping(placement)?,
            );
        }
        if placement
            .host_operations
            .iter()
            .any(|operation| resource_effect::matches(operation.contract_id.as_str()))
        {
            snapshots[usize::from(node.node.0)] = Some(Box::new(
                resource_effect::SnapshotState::prepare(placement)?,
            ));
        }
        operations.push((installation.prepare)(placement, &mut values)?);
        comparisons[usize::from(node.node.0)] =
            crate::installed_browser::pattern_comparison::prepare_codec(placement)?;
        attempts[usize::from(node.node.0)] =
            crate::installed_browser::button_attempt::prepare_codec(placement)?;
        timing[usize::from(node.node.0)] =
            crate::installed_browser::timing::PreparedTiming::for_placement(placement)?;
        if placement.host_operations.iter().any(|operation| {
            operation.contract_id.as_str()
                == crate::installed_browser::pointer_selector::HOST_OPERATION
        }) {
            selectors[usize::from(node.node.0)] =
                Some(crate::installed_browser::pointer_selector::PreparedSelector::new(placement)?);
        }
    }
    while operations.len() < MAXIMUM_BROWSER_GEARS {
        operations.push(BrowserOperation::inactive());
    }
    let drivers = operations
        .into_iter()
        .map(|operation| OperationDriver::new(operation).map_err(debug_error))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "browser operation table exceeded its admitted bound")?;

    let inactive_node = NodeSpec {
        input_cords: [None; BROWSER_PORTS_PER_GEAR],
        maximum_step_work: 1,
    };
    let mut nodes = [inactive_node; MAXIMUM_BROWSER_GEARS];
    for (destination, spec) in nodes
        .iter_mut()
        .zip(partitions.iter().flat_map(|(_, part)| &part.node_specs))
    {
        *destination = *spec;
    }
    let inactive_cord = CordSpec {
        cord: CordId(u16::MAX),
        source: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        sink: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        slot_start: u16::MAX,
        item_capacity: 0,
        byte_capacity: 0,
    };
    let mut cords = [inactive_cord; MAXIMUM_BROWSER_CORDS];
    for (destination, lowered_cord) in cords
        .iter_mut()
        .zip(partitions.iter().flat_map(|(_, part)| &part.cords))
    {
        *destination = lowered_cord.spec;
    }
    let mut routes = FixedRoutes::<BROWSER_ROUTE_SLOTS, BROWSER_ROUTE_TARGETS>::new(
        BROWSER_PORTS_PER_GEAR as u16,
    );
    for route in partitions.iter().flat_map(|(_, part)| &part.routes) {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(debug_error)?;
    }
    routes.seal().map_err(debug_error)?;
    let mut bindings = FixedHostOperationBindings::<BROWSER_HOST_OPERATION_BINDINGS>::new(
        BROWSER_HOST_OPERATIONS_PER_GEAR,
    );
    for operation in partitions
        .iter()
        .flat_map(|(_, part)| &part.host_operations)
    {
        bindings
            .install(operation.node, operation.binding)
            .map_err(debug_error)?;
    }
    bindings.seal().map_err(debug_error)?;
    let sign_bytes = u32::from(BROWSER_SIGN_ITEMS)
        .checked_mul(
            u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>())
                .map_err(|_| "browser Sign size overflow")?,
        )
        .ok_or("browser Sign budget overflow")?;
    let remote_sign_bytes = conduit_kernel::remote_sign_storage_bytes(BROWSER_SIGN_ITEMS)
        .ok_or("browser remote Sign budget overflow")?;
    let signs = HostedSignLog::new_with_remote_storage(
        BROWSER_SIGN_ITEMS,
        sign_bytes,
        BROWSER_SIGN_ITEMS,
        remote_sign_bytes,
    )
    .map_err(debug_error)?;
    let kernel = BrowserKernel::new_with_active_counts_and_host_operations(
        active_nodes,
        active_cords,
        nodes,
        cords,
        routes,
        bindings,
        drivers,
        values,
        signs,
    )
    .map_err(debug_error)?;
    Ok(TourScheduler {
        failure: None,
        kernel,
        mappings,
        selectors,
        timing,
        attempts,
        comparisons,
        snapshots,
    })
}
