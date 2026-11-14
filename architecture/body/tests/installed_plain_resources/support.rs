use super::*;
#[allow(dead_code)]
#[path = "../delegated_line_fore/support.rs"]
mod original;
pub use original::{command, policy, workset_command, Fixture};
pub fn row() -> PlainFragmentResource<'static> {
    PlainFragmentResource {
        node: 0,
        resource: 0,
        placement: "placement/installed",
        pool: "pool/installed",
        class: "test/plain",
        units: 1,
    }
}
pub fn bytes(rows: &[PlainFragmentResource<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    for row in rows {
        row.append_to(&mut out).unwrap();
    }
    out
}
pub fn table(rows: &[PlainFragmentResource<'_>]) -> InstalledPlainResourceTable {
    InstalledPlainResourceTable::new(Box::leak(bytes(rows).into_boxed_slice())).unwrap()
}
pub fn projection() -> Vec<u8> {
    let mut p = original::projection(c::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA);
    p.truncate(c::ASSIGNED_PLAN_HEADER_BYTES);
    p[80..93].fill(0);
    for (tag, data) in [
        (c::ASSIGNED_NODE, vec![0; 52]),
        (c::ASSIGNED_RESOURCE, [0, 0, 0, 0, 1, 0, 0, 0].to_vec()),
    ] {
        p[80 + usize::from(tag - 1)] += 1;
        p.push(tag);
        p.extend_from_slice(&(data.len() as u16).to_le_bytes());
        p.extend(data);
    }
    let length = p.len() as u16;
    p[10..12].copy_from_slice(&length.to_le_bytes());
    let digest = c::assigned_plan_payload_digest(&p[c::ASSIGNED_PLAN_HEADER_BYTES..]);
    p[93..125].copy_from_slice(&digest);
    p
}
pub fn profile(
    resources: InstalledPlainResourceTable,
) -> AssignedBodyInstalledPlainResourcesProfile<'static> {
    let mut preparation = original::legacy_profile(&[]);
    preparation.requirements.resources = &[0];
    AssignedBodyInstalledPlainResourcesProfile {
        preparation,
        resources,
    }
}
pub fn document(p: &[u8], reference: &[u8]) -> Vec<u8> {
    let old = original::document(p, "body/1", "boot/1", &[], &[]);
    let (mut metadata, _) = decode_body_fragment_document(&old).unwrap();
    metadata.schema = 4;
    metadata.resource_bindings = reference;
    encode_body_fragment_document(&metadata, p, 4096).unwrap()
}
pub fn actual() -> Fixture {
    let mut f = Fixture::new();
    f.host
        .resources
        .push(c::resource_offer("pool/installed", "test/plain", 4));
    f
}
pub fn observations(f: &Fixture) -> Vec<c::ResourceObservation> {
    vec![c::ResourceObservation {
        host_id: f.host.host_id.clone(),
        boot_id: f.host.boot_id.clone(),
        offer_generation: f.host.offer_generation,
        pool_id: "pool/installed".into(),
        class_id: "test/plain".into(),
        health: c::ResourceHealth::Ready,
        unreserved_units: 4,
        utilized_units: 0,
        sign_id: "sign/ready".into(),
    }]
}
