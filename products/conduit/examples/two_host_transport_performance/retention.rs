//! Typed raw kernel evidence; no debug-string substitute for identity fields.
use conduit::RemoteKernelEvidence;
use serde_json::{json, Value};
pub fn retain(host: &RemoteKernelEvidence) -> Result<Value, String> {
    if host.retention_gap.is_some() {
        return Err("kernel evidence has a retention gap".into());
    }
    let identity = &host.identity;
    if !host
        .events
        .windows(2)
        .all(|e| e[0].sequence < e[1].sequence)
    {
        return Err("kernel Sign order differs".into());
    }
    Ok(json!({
        "plan_id":identity.plan_id.as_str(),"fragment_id":identity.fragment_id.as_str(),
        "placements":identity.placements.iter().map(|(node, placement)|json!({"node":node.0,"placement":placement.as_str()})).collect::<Vec<_>>(),
        "ports":identity.ports.iter().map(|p|json!({"node":p.node.0,"direction":format!("{:?}",p.direction),"port":p.port.0,"plan_port":p.port_id.as_str()})).collect::<Vec<_>>(),
        "connections":identity.connections.iter().map(|(cord, connection)|json!({"cord":cord.0,"connection":connection.as_str()})).collect::<Vec<_>>(),
        "remote_endpoints":identity.remote_endpoints.iter().map(|(endpoint, connection)|json!({"endpoint":endpoint.0,"connection":connection.as_str()})).collect::<Vec<_>>(),
        "host_calls":identity.host_calls.iter().map(|(node, call, contract)|json!({"node":node.0,"call":call.0,"contract":contract.as_str()})).collect::<Vec<_>>(),
        "events":host.events.iter().map(|e|json!({"sequence":e.sequence,"node":e.node.0,"port":e.port.map(|p|p.0),"request":e.request.map(|r|r.0),"kind":format!("{:?}",e.kind)})).collect::<Vec<_>>(),
        "remote_identities":host.remote_identities.iter().map(|(sequence, r)|json!({"event_sequence":sequence,"endpoint":r.endpoint.0,"cord":r.cord.0,"direction":format!("{:?}",r.direction),"value_sequence":r.sequence})).collect::<Vec<_>>(),
        "retention_gap":null,
        "fore_endpoints":identity.fore_endpoints.iter().map(|p|json!({"endpoint":p.endpoint.0,"cord":p.cord.0,"front_port_id":p.front_port_id.as_str(),"direction":format!("{:?}",p.direction),"track":format!("{:?}",p.track),"value_kind":p.value_kind.as_str()})).collect::<Vec<_>>(),
        "resources":identity.resources.iter().map(|(node,resource,binding)|json!({"node":node.0,"resource":resource.0,"binding":binding})).collect::<Vec<_>>(),
        "resource_connections":identity.resource_connections.iter().map(|(node,resource,connection)|json!({"node":node.0,"resource":resource.0,"connection":connection.as_str()})).collect::<Vec<_>>()
    }))
}
