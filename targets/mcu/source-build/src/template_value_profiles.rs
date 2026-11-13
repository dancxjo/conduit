//! Exact resident artifact selection into inert physical slot profiles.
use crate::value_profiles::{self, PortSchema};
use conduit_core as c;
use conduit_embedded_build as e;

pub(super) fn emit(
    kernel: &e::GeneratedEmbeddedPlan,
    fragment: &c::PlanFragment,
    residents: &[&e::PreparedResidentExpression],
) -> Result<Option<String>, String> {
    let mut owned = Vec::new();
    for node in &kernel.nodes {
        let Some(image) = residents
            .iter()
            .find(|image| image.artifact_id().as_str() == node.artifact_id)
        else {
            continue;
        };
        if image.execution_storage() != e::ResidentExecutionStorage::SharedWorkspaceSchema {
            continue;
        }
        let placement = fragment
            .placements
            .iter()
            .find(|p| p.placement_id.as_str() == node.placement_id)
            .ok_or("schema placement absent")?;
        let mut programs = placement
            .configuration
            .iter()
            .filter(|c| c.key == "program");
        let c::ConfigurationValue::Text(text) =
            &programs.next().ok_or("schema program absent")?.value
        else {
            return Err("schema program encoding".into());
        };
        if programs.next().is_some() {
            return Err("schema program duplicate".into());
        }
        let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(text)
            .map_err(|_| "schema program")?;
        for (direction, ports, ty) in [
            (
                c::PortDirection::Input,
                &kernel.input_ports,
                &program.input_type,
            ),
            (
                c::PortDirection::Output,
                &kernel.output_ports,
                &program.output_type,
            ),
        ] {
            let mut ports = ports.iter().filter(|p| p.node == node.node);
            let port = ports.next().ok_or("schema resident port absent")?;
            if ports.next().is_some() {
                return Err("schema resident ports".into());
            }
            owned.push((
                node.node,
                port.port,
                direction,
                node.artifact_id.as_str(),
                ty.canonical_bytes().map_err(|_| "schema encoding")?,
            ));
        }
    }
    if owned.is_empty() {
        return Ok(None);
    }
    let declarations = owned
        .iter()
        .map(|(node, port, direction, artifact, schema)| PortSchema {
            node: *node,
            port: *port,
            direction: *direction,
            artifact,
            schema,
        })
        .collect::<Vec<_>>();
    let profiles = value_profiles::select(kernel, &declarations).map_err(str::to_owned)?;
    if !profiles
        .cords
        .iter()
        .chain(&profiles.steps)
        .any(|slot| matches!(slot, value_profiles::Slot::Schema { .. }))
    {
        return Ok(None);
    }
    Ok(Some(profiles.emit_rust()))
}
