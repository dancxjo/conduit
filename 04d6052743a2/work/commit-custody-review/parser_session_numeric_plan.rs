//! Exact selected mixed Plan correlation. No identity is resealed here.
use crate::parser_session_execution::ParserSessionEntry;
use conduit_ai::integer_categorical_step::{
    CATEGORICAL_STEP_IMPLEMENTATION, PreparedCategoricalStep,
};
use conduit_core::{ConfigurationValue, ConnectionTrack, Plan, PortDirection};
use conduit_plot::{CheckedGear, ExpandedAuthoringPlot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NumericPlanRefusal {
    Topology,
    Source,
    Placement,
    Cord,
    Fore,
    Seal,
}

/// Allocation-free structural admission runs before the ordinary Plan seal and
/// model-offer verifiers. Their temporary allocation must be reserved separately.
pub(crate) fn validate_numeric_plan_structure(
    expanded: &ExpandedAuthoringPlot,
    plan: &Plan,
) -> Result<[usize; 3], NumericPlanRefusal> {
    validate_numeric_plan_structure_for(
        expanded,
        plan,
        ParserSessionEntry::V2FeatureIndices,
        ParserSessionEntry::V2ScoreObservation,
    )
}
/// Exact sealed profile entries, preserving the complete original three-Gear
/// topology, ordered cords, Types and original expression bytes.
pub(crate) fn validate_numeric_plan_structure_for(
    expanded: &ExpandedAuthoringPlot,
    plan: &Plan,
    projector: ParserSessionEntry,
    wrapper: ParserSessionEntry,
) -> Result<[usize; 3], NumericPlanRefusal> {
    use NumericPlanRefusal as R;
    let source = &expanded.expanded;
    if expanded.front.inputs().len() != 1
        || expanded.front.outputs().len() != 1
        || source.gears.len() != 3
        || source.connections.len() != 2
        || !source.activations.is_empty()
        || !source.shared_pools.is_empty()
        || !source.realization_backs.is_empty()
        || expanded.abnormal_export.is_some()
        || plan.fragments.len() != 1
        || !plan.activations.is_empty()
        || !plan.activation_preparations.is_empty()
        || !plan.realization_backs.is_empty()
        || plan.source_document_id != source.source_document_id
        || plan.checked_plot_id != source.checked_plot_id
        || plan.expanded_plot_id != source.expanded_plot_id
    {
        return Err(R::Topology);
    }
    let [input] = expanded.input_bindings.as_slice() else {
        return Err(R::Fore);
    };
    let [output] = expanded.output_bindings.as_slice() else {
        return Err(R::Fore);
    };
    if input.track != ConnectionTrack::Payload || output.track != ConnectionTrack::Payload {
        return Err(R::Fore);
    }
    if input.front_port_id != expanded.front.inputs()[0].port_id
        || output.front_port_id != expanded.front.outputs()[0].port_id
    {
        return Err(R::Fore);
    }
    let fragment = &plan.fragments[0];
    if fragment.placements.len() != 3
        || fragment.connections.len() != 2
        || fragment.fore_ports.len() != 2
        || !fragment.states.is_empty()
        || !fragment.shared_pools.is_empty()
        || !fragment.execution_fusions.is_empty()
        || !fragment.realization_backs.is_empty()
    {
        return Err(R::Topology);
    }
    let first = source
        .gears
        .iter()
        .position(|g| g.gear_id == input.gear_id)
        .ok_or(R::Source)?;
    let last = source
        .gears
        .iter()
        .position(|g| g.gear_id == output.gear_id)
        .ok_or(R::Source)?;
    let middle = (0..3)
        .find(|i| *i != first && *i != last)
        .ok_or(R::Topology)?;
    if first == last {
        return Err(R::Topology);
    }
    let order = [first, middle, last];
    for (position, index) in order.iter().enumerate() {
        let gear = &source.gears[*index];
        if gear.inputs.len() != 1
            || gear.outputs.len() != 1
            || !gear.resource_ports.is_empty()
            || !gear.pool_references.is_empty()
            || !gear.terminal_transductions.is_empty()
        {
            return Err(R::Source);
        }
        if position != 1 {
            let entry = if position == 0 { projector } else { wrapper };
            validate_fixed_expression(gear, entry)?;
        }
        let mut candidates = fragment
            .placements
            .iter()
            .filter(|p| p.gear_id == gear.gear_id);
        let placement = candidates.next().ok_or(R::Placement)?;
        if candidates.next().is_some()
            || placement.kind_id != gear.kind_id
            || placement.kind_contract_revision != gear.kind_contract_revision
            || placement.inputs != gear.inputs
            || placement.outputs != gear.outputs
            || placement.configuration != gear.configuration
            || placement.semantic_contract != gear.semantic_contract
            || !placement.pool_references.is_empty()
            || !placement.terminal_transductions.is_empty()
            || !placement.authority.is_empty()
            || placement.base.is_some()
            || !placement.realization_properties.is_empty()
            || !placement.realization_characteristics.is_empty()
        {
            return Err(R::Placement);
        }
        if position != 1 && !placement.resources.is_empty() {
            return Err(R::Placement);
        }
    }
    if input.gear_port_id != source.gears[first].inputs[0].port_id
        || output.gear_port_id != source.gears[last].outputs[0].port_id
    {
        return Err(R::Fore);
    }
    // Source order is explicit: both cords are checked at their original index.
    // A Plan cannot exchange two equal-Type cords and retain this witness.
    for (i, cord) in source.connections.iter().enumerate() {
        let from = &source.gears[order[i]];
        let to = &source.gears[order[i + 1]];
        if cord.source_gear_id != from.gear_id
            || cord.sink_gear_id != to.gear_id
            || cord.source_port_id != from.outputs[0].port_id
            || cord.sink_port_id != to.inputs[0].port_id
            || cord.value_kind != from.outputs[0].value_kind
            || cord.value_kind != to.inputs[0].value_kind
            || cord.temporal != from.outputs[0].temporal
            || cord.temporal != to.inputs[0].temporal
            || cord.track != ConnectionTrack::Payload
        {
            return Err(R::Cord);
        }
        let planned = &fragment.connections[i];
        let from = fragment
            .placements
            .iter()
            .find(|p| p.gear_id == from.gear_id)
            .ok_or(R::Placement)?;
        let to = fragment
            .placements
            .iter()
            .find(|p| p.gear_id == to.gear_id)
            .ok_or(R::Placement)?;
        if planned.source_placement_id != from.placement_id
            || planned.sink_placement_id != to.placement_id
            || planned.source_port_id != cord.source_port_id
            || planned.sink_port_id != cord.sink_port_id
            || planned.value_kind != cord.value_kind
            || planned.track != cord.track
            || planned.temporal != cord.temporal
            || planned.resource.is_some()
            || planned.abnormal_kind.is_some()
            || planned.selected_line.is_some()
            || !planned.admitted_lines.is_empty()
            || planned.item_capacity != 1
            || planned.byte_capacity == 0
        {
            return Err(R::Cord);
        }
    }
    for (direction, binding, source_port) in [
        (PortDirection::Input, input, &source.gears[first].inputs[0]),
        (
            PortDirection::Output,
            output,
            &source.gears[last].outputs[0],
        ),
    ] {
        let mut candidates = fragment
            .fore_ports
            .iter()
            .filter(|p| p.direction == direction);
        let fore = candidates.next().ok_or(R::Fore)?;
        let placement = fragment
            .placements
            .iter()
            .find(|p| p.gear_id == binding.gear_id)
            .ok_or(R::Placement)?;
        let front_port = if direction == PortDirection::Input {
            &expanded.front.inputs()[0]
        } else {
            &expanded.front.outputs()[0]
        };
        if front_port.value_kind != source_port.value_kind
            || front_port.temporal != source_port.temporal
            || front_port.abnormal_kind != source_port.abnormal_kind
        {
            return Err(R::Fore);
        }
        if candidates.next().is_some()
            || fore.front_port_id != binding.front_port_id
            || fore.placement_id != placement.placement_id
            || fore.gear_port_id != binding.gear_port_id
            || fore.track != binding.track
            || fore.value_kind != source_port.value_kind
            || fore.temporal != source_port.temporal
            || fore.abnormal_kind.is_some()
            || fore.selected_line.is_some()
            || fore.item_capacity != 1
            || fore.byte_capacity == 0
        {
            return Err(R::Fore);
        }
    }
    Ok(order)
}

fn validate_fixed_expression(
    gear: &CheckedGear,
    entry: ParserSessionEntry,
) -> Result<(), NumericPlanRefusal> {
    let [configuration] = gear.configuration.as_slice() else {
        return Err(NumericPlanRefusal::Source);
    };
    let ConfigurationValue::Text(hex) = &configuration.value else {
        return Err(NumericPlanRefusal::Source);
    };
    let mut programs = entry.program_hex().lines();
    if configuration.key != "program"
        || programs.next() != Some(hex.as_str())
        || programs.next().is_some()
        || gear.kind_contract_revision.as_str() != "conduitese/pure-expression-operation@1"
        || !gear
            .kind_id
            .as_str()
            .starts_with("conduitese/pure-expression/")
    {
        return Err(NumericPlanRefusal::Source);
    }
    Ok(())
}

/// Call only after the combined model/Plan preparation peak is admitted.
pub(crate) fn validate_numeric_plan_seal_and_resource(
    expanded: &ExpandedAuthoringPlot,
    plan: &Plan,
    profile: &PreparedCategoricalStep,
) -> Result<(), NumericPlanRefusal> {
    validate_numeric_plan_seal_and_resource_for(
        expanded,
        plan,
        profile,
        ParserSessionEntry::V2FeatureIndices,
        ParserSessionEntry::V2ScoreObservation,
    )
}
pub(crate) fn validate_numeric_plan_seal_and_resource_for(
    expanded: &ExpandedAuthoringPlot,
    plan: &Plan,
    profile: &PreparedCategoricalStep,
    projector: ParserSessionEntry,
    wrapper: ParserSessionEntry,
) -> Result<(), NumericPlanRefusal> {
    let order = validate_numeric_plan_structure_for(expanded, plan, projector, wrapper)?;
    let model = &expanded.expanded.gears[order[1]];
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.gear_id == model.gear_id)
        .ok_or(NumericPlanRefusal::Placement)?;
    if placement.implementation_id.as_str() != CATEGORICAL_STEP_IMPLEMENTATION {
        return Err(NumericPlanRefusal::Placement);
    }
    for index in [order[0], order[2]] {
        let gear = &expanded.expanded.gears[index];
        let ConfigurationValue::Text(hex) = &gear.configuration[0].value else {
            return Err(NumericPlanRefusal::Source);
        };
        let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(hex)
            .map_err(|_| NumericPlanRefusal::Source)?;
        let definition =
            conduit_plot::portable_expression_definition(&program, gear.inputs[0].temporal)
                .map_err(|_| NumericPlanRefusal::Source)?;
        if gear.kind_id != definition.kind_id
            || gear.kind_contract_revision != definition.kind_contract_revision
            || gear.inputs != definition.inputs
            || gear.outputs != definition.outputs
            || gear.semantic_contract.configuration != definition.configuration
            || gear.semantic_contract.laws != conduit_plot::pure_expression_semantic_laws()
        {
            return Err(NumericPlanRefusal::Source);
        }
    }
    profile
        .verify_placement(placement, true)
        .map_err(|_| NumericPlanRefusal::Placement)?;
    if !conduit_core::verify_plan(plan) {
        return Err(NumericPlanRefusal::Seal);
    }
    Ok(())
}
