use conduit_core::PlanFragment;
use conduit_plan_lowering::lowering::LoweredPlanFragment;

use crate::model::{EmbeddedImageBounds, GenerationError};

pub(crate) fn validate_shape(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    bounds: EmbeddedImageBounds,
) -> Result<(), GenerationError> {
    if lowered.nodes.len() != lowered.node_specs.len()
        || lowered.nodes.len() != lowered.identity.placements.len()
        || lowered.nodes.len() != fragment.placements.len()
    {
        return Err(GenerationError::InconsistentLowering("node tables"));
    }
    if lowered.cords.len() != lowered.identity.connections.len()
        || lowered.cords.len() != fragment.connections.len()
    {
        return Err(GenerationError::InconsistentLowering("cord tables"));
    }
    if lowered.host_operations.len() != lowered.identity.host_operations.len()
        || lowered.resources.len() != lowered.identity.resources.len()
        || lowered.signs.len() != fragment.expected_sign.len()
    {
        return Err(GenerationError::InconsistentLowering("identity tables"));
    }

    check_bound("nodes", lowered.nodes.len(), bounds.maximum_nodes)?;
    check_bound("cords", lowered.cords.len(), bounds.maximum_cords)?;
    check_bound("routes", lowered.routes.len(), bounds.maximum_routes)?;
    check_bound(
        "host operations",
        lowered.host_operations.len(),
        bounds.maximum_host_operations,
    )?;
    check_bound(
        "resources",
        lowered.resources.len(),
        bounds.maximum_resources,
    )?;
    check_bound(
        "sign expectations",
        lowered.signs.len(),
        bounds.maximum_sign_expectations,
    )?;
    check_bound(
        "remote endpoints",
        lowered.remote_endpoints.len(),
        bounds.maximum_remote_endpoints,
    )?;
    let configuration_entries =
        fragment
            .placements
            .iter()
            .try_fold(0usize, |total, placement| {
                total.checked_add(placement.configuration.len()).ok_or(
                    GenerationError::ArithmeticOverflow("configuration entry count"),
                )
            })?;
    check_bound(
        "configuration entries",
        configuration_entries,
        bounds.maximum_configuration_entries,
    )?;
    check_numeric_bound(
        "cord value slots",
        lowered.cord_value_slots,
        bounds.maximum_cord_value_slots,
    )?;
    check_numeric_bound(
        "cord value bytes",
        lowered.cord_value_bytes,
        bounds.maximum_cord_value_bytes,
    )?;
    check_numeric_bound("sign items", lowered.sign_items, bounds.maximum_sign_items)?;
    check_numeric_bound("sign bytes", lowered.sign_bytes, bounds.maximum_sign_bytes)?;

    for node in &lowered.nodes {
        let ports = node.inputs.len().max(node.outputs.len());
        check_bound("ports per node", ports, bounds.maximum_ports_per_node)?;
        if node.maximum_step_work == 0 {
            return Err(GenerationError::InconsistentLowering(
                "zero node step-work bound",
            ));
        }
    }
    validate_cord_storage(lowered)?;
    validate_route_ranges(lowered, bounds.maximum_route_targets)
}

fn validate_cord_storage(lowered: &LoweredPlanFragment) -> Result<(), GenerationError> {
    let mut occupied_slots = vec![false; usize::from(lowered.cord_value_slots)];
    let mut byte_total = 0u32;
    for cord in &lowered.cords {
        if cord.spec.item_capacity == 0 || cord.spec.byte_capacity == 0 {
            return Err(GenerationError::InconsistentLowering("zero cord capacity"));
        }
        let end = cord
            .spec
            .slot_start
            .checked_add(cord.spec.item_capacity)
            .ok_or(GenerationError::ArithmeticOverflow("cord slot range"))?;
        validate_range(
            "cord slots",
            cord.spec.slot_start,
            cord.spec.item_capacity,
            lowered.cord_value_slots,
        )?;
        for slot in cord.spec.slot_start..end {
            let occupied = occupied_slots
                .get_mut(usize::from(slot))
                .ok_or(GenerationError::InconsistentLowering("cord slot range"))?;
            if *occupied {
                return Err(GenerationError::InconsistentLowering(
                    "overlapping cord slot ranges",
                ));
            }
            *occupied = true;
        }
        byte_total = byte_total
            .checked_add(cord.spec.byte_capacity)
            .ok_or(GenerationError::ArithmeticOverflow("cord byte total"))?;
    }
    if byte_total != lowered.cord_value_bytes || occupied_slots.iter().any(|occupied| !occupied) {
        return Err(GenerationError::InconsistentLowering(
            "cord storage accounting",
        ));
    }
    Ok(())
}

fn validate_route_ranges(
    lowered: &LoweredPlanFragment,
    maximum_targets: usize,
) -> Result<(), GenerationError> {
    let route_target_count = lowered.routes.iter().try_fold(0usize, |total, route| {
        if usize::from(route.range.len) != route.targets.len() {
            return Err(GenerationError::InconsistentLowering("route target count"));
        }
        let expected_start = u16::try_from(total)
            .map_err(|_| GenerationError::ArithmeticOverflow("route target ordinal"))?;
        if route.range.start != expected_start {
            return Err(GenerationError::InconsistentLowering(
                "non-contiguous route ranges",
            ));
        }
        total
            .checked_add(route.targets.len())
            .ok_or(GenerationError::ArithmeticOverflow("route target count"))
    })?;
    check_bound("route targets", route_target_count, maximum_targets)
}

fn check_bound(table: &'static str, actual: usize, maximum: usize) -> Result<(), GenerationError> {
    if actual > maximum {
        return Err(GenerationError::BoundExceeded {
            table,
            actual: u64::try_from(actual).unwrap_or(u64::MAX),
            maximum: u64::try_from(maximum).unwrap_or(u64::MAX),
        });
    }
    Ok(())
}

fn check_numeric_bound<T>(table: &'static str, actual: T, maximum: T) -> Result<(), GenerationError>
where
    T: Copy + Into<u64> + PartialOrd,
{
    if actual > maximum {
        return Err(GenerationError::BoundExceeded {
            table,
            actual: actual.into(),
            maximum: maximum.into(),
        });
    }
    Ok(())
}

pub(crate) fn validate_range<T>(
    table: &'static str,
    start: T,
    length: T,
    limit: T,
) -> Result<(), GenerationError>
where
    T: Copy + Into<u64>,
{
    let start = start.into();
    let length = length.into();
    let limit = limit.into();
    let end = start
        .checked_add(length)
        .ok_or(GenerationError::ArithmeticOverflow(table))?;
    if end > limit {
        return Err(GenerationError::InvalidRange {
            table,
            start,
            length,
            limit,
        });
    }
    Ok(())
}
