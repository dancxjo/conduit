//! Exact execution-region and worker-storage commitments.
use crate::{push_resource_binding, push_string, push_u32, ExecutionRegion, ExecutionScheduling};

pub(super) fn push_canonical(
    canonical: &mut impl crate::canonical_bytes::CanonicalSink,
    regions: &[ExecutionRegion],
) {
    push_u32(canonical, regions.len() as u32);
    for region in regions {
        push_string(canonical, region.region_id.as_str());
        push_u32(canonical, region.admitted_placements.len() as u32);
        for placement in &region.admitted_placements {
            push_string(canonical, placement.as_str());
        }
        push_string(canonical, region.execution_profile_id.as_str());
        match &region.scheduling {
            // Preserve the existing tags and every following byte exactly.
            ExecutionScheduling::CooperativeBoundedStep => canonical.push(0),
            ExecutionScheduling::PartitionedBoundedStep { storage } => {
                canonical.push(1);
                push_resource_binding(canonical, &storage.binding);
                push_u32(canonical, storage.stack_bytes);
                push_u32(canonical, storage.scratch_bytes);
            }
            ExecutionScheduling::ProtectedPreemptibleStep { policy, context } => {
                canonical.push(2);
                push_string(canonical, policy.accounting.profile_id());
                push_u32(canonical, policy.quantum_interrupts);
                push_u32(canonical, policy.total_interrupts);
                push_resource_binding(canonical, &context.binding);
                push_u32(canonical, context.bytes);
                push_u32(canonical, context.alignment);
            }
        }
        push_u32(canonical, region.lane_count);
        push_resource_binding(canonical, &region.lane_resource);
        push_string(canonical, region.lane_base_id.as_str());
        push_u32(canonical, region.requirements.runtime_memory_bytes);
        push_u32(canonical, region.requirements.timer_slots);
        push_u32(canonical, region.requirements.cord_item_capacity);
        push_u32(canonical, region.requirements.cord_byte_capacity);
        push_u32(
            canonical,
            u32::from(region.requirements.mandatory_sign_items),
        );
        push_u32(canonical, region.requirements.mandatory_sign_bytes);
        canonical.push(u8::from(region.preemption_required));
        canonical.push(u8::from(region.isolation_required));
    }
}

#[cfg(test)]
mod tests;
