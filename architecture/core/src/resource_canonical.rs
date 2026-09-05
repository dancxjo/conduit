use crate::{push_string, push_u32, push_u64, ResourceBinding, ResourceContentOffer};
use alloc::vec::Vec;

pub(crate) fn push_resource_binding(canonical: &mut Vec<u8>, binding: &ResourceBinding) {
    push_string(canonical, binding.pool_id.as_str());
    push_string(canonical, binding.class_id.as_str());
    push_u32(canonical, binding.units);
    if let Some(content) = &binding.content {
        canonical.extend_from_slice(b"resource-content@1");
        push_content(canonical, content);
    }
    match &binding.compute {
        Some(compute) => {
            canonical.push(1);
            push_u32(canonical, compute.selected_lanes);
            canonical.push(compute.service_guarantee as u8);
            push_string(canonical, compute.architecture_base_id.as_str());
            canonical.push(compute.architecture_base_kind as u8);
            match &compute.topology_group_id {
                Some(group) => {
                    canonical.push(1);
                    push_string(canonical, group.as_str());
                }
                None => canonical.push(0),
            }
            match &compute.performance_class {
                Some(class) => {
                    canonical.push(1);
                    push_string(canonical, class.as_str());
                }
                None => canonical.push(0),
            }
            match compute.nominal_clock_hz {
                Some(hz) => {
                    canonical.push(1);
                    push_u64(canonical, hz);
                }
                None => canonical.push(0),
            }
        }
        None => canonical.push(0),
    }
    match &binding.protected {
        Some(protected) => {
            canonical.push(1);
            push_string(canonical, protected.role_id.as_str());
            push_string(canonical, protected.handle_id.as_str());
            canonical.push(protected.access as u8);
            push_u64(canonical, protected.maximum_bytes);
            canonical.push(protected.commit_policy as u8);
        }
        None => canonical.push(0),
    }
}

fn push_content(bytes: &mut Vec<u8>, offer: &ResourceContentOffer) {
    let c = &offer.contract;
    bytes.extend_from_slice(&c.identity);
    bytes.extend_from_slice(&c.version);
    push_string(bytes, c.content_profile.as_str());
    push_u32(bytes, c.maximum_bytes);
    push_u32(bytes, c.maximum_items);
    bytes.extend_from_slice(&[
        c.retention as u8,
        c.sharing as u8,
        c.access as u8,
        u8::from(c.sensitive),
    ]);
    for bound in [c.generation_slots, c.reader_leases, c.publication_slots] {
        bytes.extend_from_slice(&bound.to_le_bytes());
    }
    for id in [
        offer.owner_host.as_str(),
        offer.owner_boot.as_str(),
        offer.base_id.as_str(),
        offer.residence_profile.as_str(),
    ] {
        push_string(bytes, id);
    }
}
