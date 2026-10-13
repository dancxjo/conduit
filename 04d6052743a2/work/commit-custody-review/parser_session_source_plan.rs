//! Correlation of a complete fixed pure Source chain with its original Plan.
//! This admits neither arbitrary caller programs nor individually valid states.
use crate::parser_session_execution::ParserSessionEntry;
use conduit_core::{ConfigurationValue, ConnectionTrack, Plan, PortDirection};
use conduit_plot::{ExpandedAuthoringPlot, PortableExpressionProgram};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourcePlanRefusal {
    Topology,
    Program,
    Placement,
    Cord,
    Fore,
    Seal,
}

/// Allocation-free topology/program admission. Complete checked Native metadata
/// parity and the separately reserved ordinary seal verifier are also required.
pub(crate) fn validate_fixed_source_plan_structure(
    expanded: &ExpandedAuthoringPlot,
    plan: &Plan,
    entry: ParserSessionEntry,
) -> Result<(), SourcePlanRefusal> {
    use SourcePlanRefusal as R;
    let source = &expanded.expanded;
    let count = source.gears.len();
    if source.name != entry.name()
        || expanded.front.inputs().len() != 1
        || expanded.front.outputs().len() != 1
        || !(1..=64).contains(&count)
        || entry.program_hex().lines().count() != count
        || source.connections.len() != count - 1
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
    if fragment.placements.len() != count
        || fragment.connections.len() != count - 1
        || fragment.fore_ports.len() != 2
        || !fragment.states.is_empty()
        || !fragment.shared_pools.is_empty()
        || !fragment.execution_fusions.is_empty()
        || !fragment.realization_backs.is_empty()
    {
        return Err(R::Topology);
    }
    let mut seen = [false; 64];
    let mut current = &input.gear_id;
    let mut programs = entry.program_hex().lines();
    for position in 0..count {
        let index = source
            .gears
            .iter()
            .position(|g| &g.gear_id == current)
            .ok_or(R::Topology)?;
        if seen[index] {
            return Err(R::Topology);
        }
        seen[index] = true;
        let gear = &source.gears[index];
        if gear.inputs.len() != 1
            || gear.outputs.len() != 1
            || !gear.resource_ports.is_empty()
            || !gear.pool_references.is_empty()
            || !gear.terminal_transductions.is_empty()
            || gear.kind_contract_revision.as_str() != "conduitese/pure-expression-operation@1"
            || !gear
                .kind_id
                .as_str()
                .starts_with("conduitese/pure-expression/")
        {
            return Err(R::Program);
        }
        let [configuration] = gear.configuration.as_slice() else {
            return Err(R::Program);
        };
        let ConfigurationValue::Text(hex) = &configuration.value else {
            return Err(R::Program);
        };
        if configuration.key != "program" || programs.next() != Some(hex.as_str()) {
            return Err(R::Program);
        }
        let mut placements = fragment
            .placements
            .iter()
            .filter(|p| p.gear_id == gear.gear_id);
        let placement = placements.next().ok_or(R::Placement)?;
        if placements.next().is_some()
            || placement.kind_id != gear.kind_id
            || placement.kind_contract_revision != gear.kind_contract_revision
            || placement.configuration != gear.configuration
            || placement.inputs != gear.inputs
            || placement.outputs != gear.outputs
            || placement.semantic_contract != gear.semantic_contract
            || !placement.resources.is_empty()
            || !placement.authority.is_empty()
            || placement.base.is_some()
            || !placement.pool_references.is_empty()
            || !placement.realization_properties.is_empty()
            || !placement.realization_characteristics.is_empty()
            || !placement.terminal_transductions.is_empty()
        {
            return Err(R::Placement);
        }
        if position == 0 && input.gear_port_id != gear.inputs[0].port_id {
            return Err(R::Fore);
        }
        let mut outgoing = source
            .connections
            .iter()
            .filter(|c| &c.source_gear_id == current);
        if position + 1 == count {
            if current != &output.gear_id
                || output.gear_port_id != gear.outputs[0].port_id
                || outgoing.next().is_some()
            {
                return Err(R::Fore);
            }
        } else {
            let cord = outgoing.next().ok_or(R::Cord)?;
            if outgoing.next().is_some() {
                return Err(R::Cord);
            }
            let sink = source
                .gears
                .iter()
                .find(|g| g.gear_id == cord.sink_gear_id)
                .ok_or(R::Cord)?;
            if sink.inputs.len() != 1
                || cord.source_port_id != gear.outputs[0].port_id
                || cord.sink_port_id != sink.inputs[0].port_id
                || cord.track != ConnectionTrack::Payload
                || cord.value_kind != gear.outputs[0].value_kind
                || cord.value_kind != sink.inputs[0].value_kind
                || cord.temporal != gear.outputs[0].temporal
                || cord.temporal != sink.inputs[0].temporal
            {
                return Err(R::Cord);
            }
            current = &cord.sink_gear_id;
        }
    }
    // Preserve original Source cord ordering independently from chain traversal.
    for (source_cord, planned) in source.connections.iter().zip(&fragment.connections) {
        let from = fragment
            .placements
            .iter()
            .find(|p| p.gear_id == source_cord.source_gear_id)
            .ok_or(R::Cord)?;
        let to = fragment
            .placements
            .iter()
            .find(|p| p.gear_id == source_cord.sink_gear_id)
            .ok_or(R::Cord)?;
        if planned.source_placement_id != from.placement_id
            || planned.sink_placement_id != to.placement_id
            || planned.source_port_id != source_cord.source_port_id
            || planned.sink_port_id != source_cord.sink_port_id
            || planned.value_kind != source_cord.value_kind
            || planned.temporal != source_cord.temporal
            || planned.track != source_cord.track
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
    for (direction, binding) in [
        (PortDirection::Input, input),
        (PortDirection::Output, output),
    ] {
        let mut ports = fragment
            .fore_ports
            .iter()
            .filter(|p| p.direction == direction);
        let fore = ports.next().ok_or(R::Fore)?;
        let gear = source
            .gears
            .iter()
            .find(|g| g.gear_id == binding.gear_id)
            .ok_or(R::Fore)?;
        let placement = fragment
            .placements
            .iter()
            .find(|p| p.gear_id == binding.gear_id)
            .ok_or(R::Fore)?;
        let source_port = if direction == PortDirection::Input {
            &gear.inputs[0]
        } else {
            &gear.outputs[0]
        };
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
        if ports.next().is_some()
            || fore.front_port_id != binding.front_port_id
            || fore.gear_port_id != binding.gear_port_id
            || fore.placement_id != placement.placement_id
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
    Ok(())
}
/// All temporary decoding/definition/seal allocations must already be reserved.
pub(crate) fn validate_fixed_source_plan_seal(
    expanded: &ExpandedAuthoringPlot,
    plan: &Plan,
    entry: ParserSessionEntry,
) -> Result<(), SourcePlanRefusal> {
    use SourcePlanRefusal as R;
    validate_fixed_source_plan_structure(expanded, plan, entry)?;
    for gear in &expanded.expanded.gears {
        let ConfigurationValue::Text(hex) = &gear.configuration[0].value else {
            return Err(R::Program);
        };
        let program = PortableExpressionProgram::from_canonical_hex(hex).map_err(|_| R::Program)?;
        let definition =
            conduit_plot::portable_expression_definition(&program, gear.inputs[0].temporal)
                .map_err(|_| R::Program)?;
        if definition.kind_id != gear.kind_id
            || definition.kind_contract_revision != gear.kind_contract_revision
            || definition.inputs != gear.inputs
            || definition.outputs != gear.outputs
            || definition.configuration != gear.semantic_contract.configuration
            || conduit_plot::pure_expression_semantic_laws() != gear.semantic_contract.laws
        {
            return Err(R::Program);
        }
    }
    if !conduit_core::verify_plan(plan) {
        return Err(R::Seal);
    }
    Ok(())
}
