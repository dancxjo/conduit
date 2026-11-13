//! A selected standard software artifact promises schema support, not its Kind.
use crate::{software, value_profiles::PortSchema};
use conduit_core as c;
use conduit_embedded_build as e;
pub(super) type OwnedSchema<'a> = (u16, u16, c::PortDirection, &'a str, Vec<u8>);

pub(super) fn append<'a>(
    owned: &mut Vec<OwnedSchema<'a>>,
    kernel: &'a e::GeneratedEmbeddedPlan,
    fragment: &c::PlanFragment,
    source: &conduit_plot::ExpandedCanonicalPlot,
) -> Result<(), String> {
    for node in &kernel.nodes {
        if node.artifact_id != software::ARTIFACT {
            continue;
        }
        let placement = fragment
            .placements
            .iter()
            .find(|p| p.placement_id.as_str() == node.placement_id)
            .ok_or("selector placement absent")?;
        let gear = source
            .gears
            .iter()
            .find(|g| g.gear_id == placement.gear_id)
            .ok_or("selector Source Gear absent")?;
        let Some(types) = selected_types(node, &placement.configuration, gear)? else {
            continue;
        };
        for (direction, ports, ty) in [
            (c::PortDirection::Input, &kernel.input_ports, &types[0]),
            (c::PortDirection::Output, &kernel.output_ports, &types[1]),
        ] {
            if matches!(ty.shape(), c::StructuredInfoTypeShape::Leaf(k) if k.as_str()=="value/empty")
            {
                continue;
            }
            let mut matching = ports.iter().filter(|p| p.node == node.node);
            let port = matching.next().ok_or("selector schema port absent")?;
            if matching.next().is_some() || port.port != 0 {
                return Err("selector schema port geometry".into());
            }
            owned.push((
                node.node,
                port.port,
                direction,
                node.artifact_id.as_str(),
                ty.canonical_bytes()
                    .map_err(|_| "selector schema encoding")?,
            ));
        }
    }
    Ok(())
}
pub(super) fn declarations<'a>(owned: &'a [OwnedSchema<'_>]) -> Vec<PortSchema<'a>> {
    owned
        .iter()
        .map(|(node, port, direction, artifact, schema)| PortSchema {
            node: *node,
            port: *port,
            direction: *direction,
            artifact,
            schema,
        })
        .collect()
}

fn selected_types(
    node: &e::GeneratedStaticNode,
    configuration: &[c::ConfigurationEntry],
    gear: &conduit_plot::CheckedGear,
) -> Result<Option<[c::StructuredInfoType; 2]>, String> {
    if node.artifact_id != software::ARTIFACT {
        return Ok(None);
    }
    let Some((kind, software::SoftwareRole::Selector { canonical, .. })) =
        software::selected_selector(gear)?
    else {
        return Ok(None);
    };
    if software::implementation_identity(&kind.kind_id, &gear.configuration)?.as_str()
        != node.implementation_id
    {
        return Ok(None);
    }
    if configuration != gear.configuration
        || node.kind_id != kind.kind_id.as_str()
        || kind.kind_contract_revision != gear.kind_contract_revision
        || kind.startup_parameters != gear.startup_parameters
        || kind.shorthand != gear.shorthand
        || kind.inputs != gear.inputs
        || kind.outputs != gear.outputs
        || kind.semantic_contract() != gear.semantic_contract
    {
        return Err("selector schema differs from checked realization".into());
    }
    let selector = c::StructuredSelector::from_canonical_bytes(&canonical)
        .map_err(|_| "selector schema material")?;
    Ok(Some([
        selector.input_type().clone(),
        selector.output_type().clone(),
    ]))
}
#[cfg(test)]
#[path = "template_selector_profile_tests.rs"]
mod tests;
