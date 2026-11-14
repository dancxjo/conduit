//! Exact standard prepared Zip ports; Type custody is supplied by checked Source.
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
        return Err("Zip schema Source custody differs".into());
    }
    for node in &kernel.nodes {
        if node.artifact_id != software::ARTIFACT {
            continue;
        }
        let placement = fragment
            .placements
            .iter()
            .find(|p| p.placement_id.as_str() == node.placement_id)
            .ok_or("Zip schema placement absent")?;
        let gear = source
            .gears
            .iter()
            .find(|g| g.gear_id == placement.gear_id)
            .ok_or("Zip schema Source Gear absent")?;
        let Some((kind, composite::Role::Zip { left, right, .. })) =
            composite::selected(gear, Some(types))?
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
            || !left.contract.constraints.is_empty()
            || !right.contract.constraints.is_empty()
        {
            return Err("Zip schema differs from checked realization".into());
        }
        let output = c::tuple_info_type(vec![left.schema.clone(), right.schema.clone()])
            .map_err(|_| "Zip output schema")?;
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
                return Err("Zip schema port geometry".into());
            }
            for port in selected {
                let ty = match (direction, port.port) {
                    (c::PortDirection::Input, 0) => &left.schema,
                    (c::PortDirection::Input, 1) => &right.schema,
                    (c::PortDirection::Output, 0) => &output,
                    _ => return Err("Zip schema port".into()),
                };
                // Primitive inputs use raw bytes; the pair output is structured.
                if matches!(ty.shape(), c::StructuredInfoTypeShape::Leaf(_)) {
                    continue;
                }
                owned.push((
                    node.node,
                    port.port,
                    direction,
                    node.artifact_id.as_str(),
                    ty.canonical_bytes().map_err(|_| "Zip schema encoding")?,
                ));
            }
        }
    }
    Ok(())
}
