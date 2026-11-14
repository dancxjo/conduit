//! Exact standard linked Merge ports; Type custody is supplied by checked Source.
use super::selectors::OwnedSchema;
use crate::{composite, software};
use conduit_core as c;
use conduit_embedded_build as e;
pub(super) fn append<'a>(
    owned: &mut Vec<OwnedSchema<'a>>,
    kernel: &'a e::GeneratedEmbeddedPlan,
    fragment: &c::PlanFragment,
    source: &conduit_plot::ExpandedCanonicalPlot,
    types: Option<&conduit_plot::CheckedSyntaxDocument>,
) -> Result<(), String> {
    let Some(types) = types else {
        return Ok(());
    };
    if types.source_document_id != source.source_document_id {
        return Err("Merge schema Source custody differs".into());
    }
    for node in &kernel.nodes {
        if node.artifact_id != software::ARTIFACT {
            continue;
        }
        let placement = fragment
            .placements
            .iter()
            .find(|p| p.placement_id.as_str() == node.placement_id)
            .ok_or("Merge schema placement absent")?;
        let gear = source
            .gears
            .iter()
            .find(|g| g.gear_id == placement.gear_id)
            .ok_or("Merge schema Source Gear absent")?;
        let Some((kind, composite::Role::Merge(value))) = composite::selected(gear, Some(types))?
        else {
            continue;
        };
        if software::implementation_identity(&kind.kind_id, &gear.configuration)?.as_str()
            != node.implementation_id
        {
            continue;
        }
        if placement.configuration != gear.configuration
            || node.kind_id != kind.kind_id.as_str()
            || kind.kind_contract_revision != gear.kind_contract_revision
            || kind.startup_parameters != gear.startup_parameters
            || kind.shorthand != gear.shorthand
            || kind.inputs != gear.inputs
            || kind.outputs != gear.outputs
            || kind.semantic_contract() != gear.semantic_contract
            || !value.contract.constraints.is_empty()
        {
            return Err("Merge schema differs from checked realization".into());
        }
        // The ordinary Merge leaf ABI carries raw bytes, not a structured pair.
        if matches!(value.schema.shape(), c::StructuredInfoTypeShape::Leaf(_)) {
            continue;
        }
        let schema = value
            .schema
            .canonical_bytes()
            .map_err(|_| "Merge schema encoding")?;
        for (direction, ports, count) in [
            (c::PortDirection::Input, &kernel.input_ports, 2u16),
            (c::PortDirection::Output, &kernel.output_ports, 1u16),
        ] {
            let selected = ports
                .iter()
                .filter(|p| p.node == node.node)
                .collect::<Vec<_>>();
            if selected.len() != usize::from(count)
                || (0..count).any(|id| selected.iter().filter(|p| p.port == id).count() != 1)
            {
                return Err("Merge schema port geometry".into());
            }
            for port in selected {
                owned.push((
                    node.node,
                    port.port,
                    direction,
                    node.artifact_id.as_str(),
                    schema.clone(),
                ));
            }
        }
    }
    Ok(())
}
