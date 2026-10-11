//! Exact authority and admitted member-session Lines for a pool envelope.
use super::*;
use conduit_core::{
    AdmittedLine, LineAvailability, SHARED_POOL_ADMIT_AUTHORITY_CONTRACT,
    SHARED_POOL_ADMIT_HOST_CALL_CONTRACT, SHARED_POOL_AUTHORITY_SUBJECT_KIND,
};

pub(super) fn validate_pool_authority(
    grant: &AuthorityGrant,
    hosts: &[HostAdvertisement],
) -> Result<(), PlannerError> {
    let exact_scope = grant.contract_id.as_str() == SHARED_POOL_ADMIT_AUTHORITY_CONTRACT
        && grant.host_call_contract_id.as_str() == SHARED_POOL_ADMIT_HOST_CALL_CONTRACT
        && grant.subject_kind.as_str() == SHARED_POOL_AUTHORITY_SUBJECT_KIND
        && !grant.grant_id.as_str().is_empty()
        && hosts.iter().any(|host| {
            host.host_id == grant.host_id
                && host.boot_id == grant.boot_id
                && host
                    .capabilities
                    .iter()
                    .any(|capability| capability.capability_id == grant.capability_id)
        });
    if !exact_scope {
        return Err(PlannerError::InvalidSharedPool(
            "shared-pool admission authority is missing or has the wrong exact scope".into(),
        ));
    }
    Ok(())
}

pub(super) fn pool_member_lines(
    member_host: &HostAdvertisement,
    member_front: &conduit_core::CheckedFront,
    consumers: &[&PlannedGear],
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    requirement: &SharedPoolPlanningRequirement,
) -> Result<Vec<AdmittedLine>, PlannerError> {
    if !requirement.member_sessions_required {
        return Ok(Vec::new());
    }

    let line_policy = crate::contract::LineMechanismPolicy::new(bases);
    let mut admitted = Vec::new();
    let mut identities = BTreeSet::new();
    for consumer in consumers {
        if consumer.host_id == member_host.host_id && consumer.boot_id == member_host.boot_id {
            continue;
        }
        let mut directions = Vec::with_capacity(2);
        if !member_front.inputs().is_empty() {
            directions.push((
                &consumer.host_id,
                &consumer.boot_id,
                &member_host.host_id,
                &member_host.boot_id,
                "consumer-to-member",
            ));
        }
        if !member_front.outputs().is_empty() {
            directions.push((
                &member_host.host_id,
                &member_host.boot_id,
                &consumer.host_id,
                &consumer.boot_id,
                "member-to-consumer",
            ));
        }
        for (source_host, source_boot, sink_host, sink_boot, direction) in directions {
            let matches = options
                .line_offers
                .iter()
                .filter(|offer| {
                    offer.binding.source.host_id == *source_host
                        && offer.binding.source.boot_id == *source_boot
                        && offer.binding.sink.host_id == *sink_host
                        && offer.binding.sink.boot_id == *sink_boot
                        && line_policy.permits_remote(&offer.binding.base)
                        && offer.validate_sign_identity()
                        && offer.availability.availability == LineAvailability::Ready
                        && offer.binding.limits.maximum_in_flight_items
                            >= requirement.member_limits.queue_item_capacity
                        && offer.binding.limits.maximum_payload_bytes
                            >= requirement.member_limits.queue_byte_capacity
                        && offer.binding.limits.maximum_buffered_bytes
                            >= requirement.member_limits.queue_byte_capacity
                        && offer.binding.limits.maximum_frame_bytes
                            >= offer.binding.limits.maximum_payload_bytes
                })
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(PlannerError::InvalidSharedPool(format!(
                    "shared pool member session requires one exact ready bounded {direction} Line between '{}@{}' and '{}@{}'; found {}",
                    source_host.as_str(), source_boot.as_str(), sink_host.as_str(), sink_boot.as_str(), matches.len()
                )));
            }
            let line = matches[0].admitted_line();
            if !identities.insert((line.line_id.clone(), line.binding.binding_id.clone())) {
                return Err(PlannerError::InvalidSharedPool(
                    "shared pool member session repeats an admitted Line identity".into(),
                ));
            }
            admitted.push(line);
        }
    }
    Ok(admitted)
}
