//! Test-only existing schema6 encoder; no Make/provider authority is claimed.
use super::*;
fn id(s: &str) -> AssignedIdentity {
    AssignedIdentity::from_text(s)
}
fn row(out: &mut Vec<u8>, tag: u8, payload: &[u8]) {
    out[80 + usize::from(tag - 1)] += 1;
    out.push(tag);
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.extend_from_slice(payload);
}
pub fn project(
    f: &Fixture,
    mut change_read: impl FnMut(&mut [u8; 46], &mut AssignedNativeBinding),
) -> Vec<u8> {
    let gear = &f.fragment.placements[0];
    let base = gear.base.as_ref().unwrap();
    let mut out = vec![0; ASSIGNED_PLAN_HEADER_BYTES];
    out[..8].copy_from_slice(&assigned_plan_magic());
    out[8..10].copy_from_slice(&ASSIGNED_NATIVE_PLAN_SCHEMA.to_le_bytes());
    out[12..14].copy_from_slice(&128u16.to_le_bytes());
    for (offset, text) in [
        (16, f.fragment.plan_id.as_str()),
        (32, f.fragment.fragment_id.as_str()),
        (48, f.fragment.host_id.as_str()),
        (64, f.fragment.boot_id.as_str()),
    ] {
        out[offset..offset + 16].copy_from_slice(&id(text).0);
    }
    let mut node = [0; 52];
    node[2..4].copy_from_slice(&8u16.to_le_bytes());
    node[4..20].copy_from_slice(&id(f.kind.kind_id.as_str()).0);
    node[20..36].copy_from_slice(&id(gear.implementation_id.as_str()).0);
    node[36..52].copy_from_slice(&id(gear.artifact_id.as_str()).0);
    row(&mut out, ASSIGNED_NODE, &node);
    for (direction, ports) in [(0, &f.kind.inputs), (1, &f.kind.outputs)] {
        for (ordinal, p) in ports.iter().enumerate() {
            let mut port = [0; 37];
            port[2..4].copy_from_slice(&(ordinal as u16).to_le_bytes());
            port[4] = direction;
            port[5..21].copy_from_slice(&id(p.port_id.as_str()).0);
            port[21..37].copy_from_slice(&id(p.value_kind.as_str()).0);
            row(&mut out, ASSIGNED_PORT, &port);
        }
    }
    for (ordinal, s) in f.scopes.iter().enumerate() {
        let mut prefix = [0; 46];
        prefix[2..4].copy_from_slice(&(ordinal as u16).to_le_bytes());
        prefix[4..20].copy_from_slice(&id(s.operation_contract_id.as_str()).0);
        prefix[20..36].copy_from_slice(&id(s.subject_kind.as_str()).0);
        prefix[36..38].copy_from_slice(&1u16.to_le_bytes());
        prefix[38..42].copy_from_slice(&8u32.to_le_bytes());
        prefix[42..46].copy_from_slice(&8u32.to_le_bytes());
        let mut b = AssignedNativeBinding {
            capability: id(gear.capability_id.as_str()),
            kind_revision: id(f.kind.kind_contract_revision.as_str()),
            execution_profile: id(gear.execution_profile_id.as_str()),
            maximum_instances: f.kind.limits.max_active_instances,
            queue_items: f.kind.limits.max_queue_items,
            queue_bytes: f.kind.limits.max_queue_bytes,
            base: id(base.base_id.as_str()),
            instance: id(s.base_instance_id.as_str()),
            generation: s.base_provider_generation,
            base_implementation: id(base.implementation_id.as_str()),
            mechanism: id(base.mechanism_family.as_str()),
            enforcement: 0,
            grant: id(s.authority_grant_id.as_str()),
            authority: id(s.authority_contract_id.as_str()),
            call: id(s.operation_contract_id.as_str()),
            subject: id(s.subject_kind.as_str()),
            host: id(s.host_id.as_str()),
            boot: id(s.boot_id.as_str()),
            authority_capability: id(s.capability_id.as_str()),
            resource: 0,
            pool: id(s.resource_pool_id.as_str()),
            resource_class: id(f.resource.class_id.as_str()),
            units: 1,
        };
        if ordinal == 1 {
            change_read(&mut prefix, &mut b);
        }
        let mut call = [0; ASSIGNED_NATIVE_CALL_BYTES];
        encode_assigned_native_binding(&prefix, Some(&b), &mut call).unwrap();
        row(&mut out, ASSIGNED_HOST_CALL, &call);
    }
    let mut resource = [0; 8];
    resource[4..8].copy_from_slice(&1u32.to_le_bytes());
    row(&mut out, ASSIGNED_RESOURCE, &resource);
    let length = out.len() as u16;
    out[10..12].copy_from_slice(&length.to_le_bytes());
    let digest = assigned_plan_payload_digest(&out[ASSIGNED_PLAN_HEADER_BYTES..]);
    out[93..125].copy_from_slice(&digest);
    out
}
