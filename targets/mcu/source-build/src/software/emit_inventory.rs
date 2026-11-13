//! Cold ordinal access to immutable selected software facts. Array conveniences
//! delegate to these exact lookups; no owner or authority is initialized here.
use super::*;
use std::fmt::Write;

pub(super) fn append(code: &mut String, material: &SoftwareMaterial) -> Result<(), String> {
    code.push_str("pub fn definition_at(index:usize,waits:&[(u16,conduit_kernel::ValueRef)])->Result<Option<conduit_mcu_host_runtime::execution_factory::SoftwareDefinition<'static>>,&'static str>{\nuse conduit_mcu_host_runtime::execution_factory::SoftwareDefinition as D;\nmatch index{\n");
    for node in &material.nodes {
        let definition = match &node.role {
            SoftwareRole::Installed { binding, .. } => format!("D::LinkedSourceStatic{{schema:super::installed_values::SCHEMA_{binding},value:super::installed_values::VALUE_{binding},maximum_bytes:super::installed_values::MAXIMUM_{binding}}}"),
            SoftwareRole::Tick { count, .. } => format!("D::Tick{{wait:waits.iter().find(|v|v.0=={}).ok_or(\"missing-clock-value\")?.1,count:{count}}}",node.node),
            SoftwareRole::Composite(role) => crate::composite::emit_definition(role)?,
            SoftwareRole::Trigger => "D::Trigger".into(),
            SoftwareRole::Selector { canonical, input_bytes, output_bytes } => format!("D::SelectorStatic{{canonical:&{canonical:?},input_bytes:{input_bytes},output_bytes:{output_bytes}}}"),
            SoftwareRole::Dht11Observation => "D::Dht11Observation".into(),
            SoftwareRole::RecordingLog { maximum_samples } => format!("D::RecordingLog{{maximum_samples:{maximum_samples}}}"),
            SoftwareRole::Publication { channel, maximum_events } => format!("D::Publication{{channel:conduit_environment_runtime::publication::PublicationChannel::{channel:?},maximum_events:{maximum_events}}}"),
        };
        writeln!(code, "{}=>Ok(Some({definition})),", node.node).unwrap();
    }
    code.push_str("_=>if index<NODE_COUNT{Ok(None)}else{Err(\"software-node\")},}}\n");
    code.push_str("pub fn definitions(waits:&[(u16,conduit_kernel::ValueRef)])->Result<[Option<conduit_mcu_host_runtime::execution_factory::SoftwareDefinition<'static>>;NODE_COUNT],&'static str>{let mut result=core::array::from_fn(|_|None);for(index,slot)in result.iter_mut().enumerate(){*slot=definition_at(index,waits)?;}Ok(result)}\n");
    code.push_str("pub static SOFTWARE_IDENTITIES:&[conduit_plan_lowering::resident_inventory::OtherNodeIdentity]=&[\n");
    for node in &material.nodes {
        if let (Some(implementation), Some(artifact)) = (&node.implementation, &node.artifact) {
            writeln!(code,"conduit_plan_lowering::resident_inventory::OtherNodeIdentity{{node:{},kind:conduit_core::AssignedIdentity({:?}),implementation:conduit_core::AssignedIdentity({:?}),artifact:conduit_core::AssignedIdentity({:?})}},",node.node,c::AssignedIdentity::from_text(node.kind.as_str()).0,c::AssignedIdentity::from_text(implementation.as_str()).0,c::AssignedIdentity::from_text(artifact.as_str()).0).unwrap();
        }
    }
    code.push_str("];\npub fn identity_at(index:usize,clocks:&[conduit_plan_lowering::resident_inventory::OtherNodeIdentity])->Result<Option<conduit_plan_lowering::resident_inventory::OtherNodeIdentity>,&'static str>{if index>=NODE_COUNT{return Err(\"software-node\");}match index{\n");
    for node in &material.nodes {
        if node.implementation.is_none() && matches!(node.role, SoftwareRole::Tick { .. }) {
            writeln!(code,"{}=>{{let clock=*clocks.iter().find(|v|usize::from(v.node)==index).ok_or(\"missing-clock-owner\")?;if clock.kind!=conduit_core::AssignedIdentity({:?}){{return Err(\"clock-kind\");}}Ok(Some(clock))}},",node.node,c::AssignedIdentity::from_text(node.kind.as_str()).0).unwrap();
        }
    }
    code.push_str("_=>Ok(SOFTWARE_IDENTITIES.iter().copied().find(|identity|usize::from(identity.node)==index)),}}\n");
    code.push_str("pub fn identities(clocks:&[conduit_plan_lowering::resident_inventory::OtherNodeIdentity])->Result<[Option<conduit_plan_lowering::resident_inventory::OtherNodeIdentity>;NODE_COUNT],&'static str>{let mut result=core::array::from_fn(|_|None);for(index,slot)in result.iter_mut().enumerate(){*slot=identity_at(index,clocks)?;}Ok(result)}\n");
    Ok(())
}

#[cfg(test)]
#[path = "emit_inventory_tests.rs"]
mod tests;
