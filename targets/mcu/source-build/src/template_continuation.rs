//! Immutable current-Boot substitution pins from the SAME checked Plan/lowering.
use super::*;
use std::fmt::Write;
#[derive(Debug, PartialEq, Eq)]
pub struct ForePin {
    pub cord: u16,
    pub endpoint: u16,
    pub input: bool,
    pub node: u16,
    pub port: u16,
    pub front: String,
    pub placement: String,
    pub gear: String,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ResourcePin {
    pub node: u16,
    pub resource: u16,
    pub placement: String,
    pub pool: String,
    pub class: String,
    pub units: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ContinuationPins {
    pub fore: Vec<ForePin>,
    pub resources: Vec<ResourcePin>,
}
pub(super) fn capture(
    fragment: &c::PlanFragment,
    kernel: &e::GeneratedEmbeddedPlan,
    fore: &[c::AssignedForeRequirement],
) -> Result<ContinuationPins, String> {
    let mut rows = Vec::new();
    for pin in fore {
        let input = pin.direction() == c::AssignedForeDirection::Input;
        let mut selected = fragment.fore_ports.iter().filter(|p| {
            c::AssignedIdentity::from_text(p.front_port_id.as_str()).0 == pin.payload()[5..21]
                && (p.direction == c::PortDirection::Input) == input
                && p.track == c::ConnectionTrack::Payload
        });
        let planned = selected
            .next()
            .ok_or("continuation lost exact named Fore")?;
        if selected.next().is_some() {
            return Err("continuation ambiguous named Fore".into());
        }
        let cord = kernel
            .cords
            .iter()
            .find(|c| c.cord == pin.cord())
            .ok_or("continuation lost Fore Cord")?;
        let (remote, local) = if input {
            (cord.source, cord.sink)
        } else {
            (cord.sink, cord.source)
        };
        let (
            e::GeneratedCordEndpoint::Remote { endpoint },
            e::GeneratedCordEndpoint::Local { node, port },
        ) = (remote, local)
        else {
            return Err("continuation Fore is not exact local boundary".into());
        };
        let node_pin = kernel
            .nodes
            .iter()
            .find(|n| n.node == node)
            .ok_or("Fore node")?;
        let ports = if input {
            &kernel.input_ports
        } else {
            &kernel.output_ports
        };
        if endpoint != pin.endpoint()
            || node_pin.placement_id != planned.placement_id.as_str()
            || !ports.iter().any(|p| {
                p.node == node && p.port == port && p.port_id == planned.gear_port_id.as_str()
            })
        {
            return Err("continuation Fore differs from actual checked lowering".into());
        }
        rows.push(ForePin {
            cord: pin.cord(),
            endpoint,
            input,
            node,
            port,
            front: planned.front_port_id.as_str().into(),
            placement: planned.placement_id.as_str().into(),
            gear: planned.gear_port_id.as_str().into(),
        });
    }
    let mut resources = Vec::new();
    for pin in &kernel.resources {
        let node = kernel
            .nodes
            .iter()
            .find(|n| n.node == pin.node)
            .ok_or("resource node")?;
        let placement = fragment
            .placements
            .iter()
            .find(|p| p.placement_id.as_str() == node.placement_id)
            .ok_or("resource placement")?;
        let binding = placement
            .resources
            .get(usize::from(pin.resource))
            .ok_or("resource ordinal")?;
        if binding.units != pin.units || binding.protected.is_some() || binding.compute.is_some() {
            return Err("continuation unsupported or inconsistent resource".into());
        }
        resources.push(ResourcePin {
            node: pin.node,
            resource: pin.resource,
            placement: placement.placement_id.as_str().into(),
            pool: binding.pool_id.as_str().into(),
            class: binding.class_id.as_str().into(),
            units: binding.units,
        });
    }
    Ok(ContinuationPins {
        fore: rows,
        resources,
    })
}
impl ContinuationPins {
    pub fn emit(&self) -> String {
        let mut text =
            String::from("pub static FRESH_FORE:&[conduit_core::FreshAssignedFore<'static>]=&[\n");
        for p in &self.fore {
            writeln!(text,"conduit_core::FreshAssignedFore{{cord:{},endpoint:{},direction:conduit_core::AssignedForeDirection::{},local_node:{},local_port:{},front_port:{:?},placement:{:?},gear_port:{:?}}},",
                p.cord,p.endpoint,if p.input {"Input"} else {"Output"},p.node,p.port,p.front,p.placement,p.gear).unwrap();
        }
        text.push_str("];\npub static CONTINUATION_RESOURCES:&[(u16,u16,&str,&str,&str,u32)]=&[\n");
        for p in &self.resources {
            writeln!(
                text,
                "({},{},{:?},{:?},{:?},{}),",
                p.node, p.resource, p.placement, p.pool, p.class, p.units
            )
            .unwrap();
        }
        text.push_str("];\n");
        text
    }
}
