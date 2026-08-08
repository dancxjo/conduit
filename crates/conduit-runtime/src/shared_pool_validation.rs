use conduit_core::{
    AuthorityGrant, FailureReason, HostAdvertisement, PlannedSharedPool,
    SHARED_POOL_ADMIT_AUTHORITY_CONTRACT, SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT,
    SHARED_POOL_AUTHORITY_SUBJECT_KIND,
};

pub(crate) fn validate_local_shared_pools(
    pools: &[PlannedSharedPool],
    advertisement: &HostAdvertisement,
    authority_grants: &[AuthorityGrant],
) -> Result<(), (FailureReason, String)> {
    for pool in pools {
        let grant = authority_grants
            .iter()
            .filter(|grant| grant.grant_id == pool.admission_authority)
            .collect::<Vec<_>>();
        if grant.len() != 1 || !valid_admission_grant(grant[0]) {
            return Err((
                FailureReason::AuthorityContractMismatch,
                format!(
                    "shared pool '{}' lacks its exact current admission grant",
                    pool.pool_id.as_str()
                ),
            ));
        }

        for realization in pool.realization_envelope.iter().filter(|realization| {
            realization.host_id == advertisement.host_id
                && realization.boot_id == advertisement.boot_id
        }) {
            let capability = advertisement
                .capabilities
                .iter()
                .filter(|offer| offer.capability_id == realization.capability_id)
                .collect::<Vec<_>>();
            if capability.len() != 1 {
                return Err(contract_mismatch(&pool.pool_id, "capability is absent"));
            }
            let capability = capability[0];
            if capability.checked_face() != pool.member_face
                || capability.limits.max_active_instances < realization.member_capacity
                || capability.limits.max_queue_items < pool.member_limits.queue_item_capacity
                || capability.limits.max_queue_bytes < pool.member_limits.queue_byte_capacity
            {
                return Err(contract_mismatch(
                    &pool.pool_id,
                    "capability face or finite limits drifted",
                ));
            }
            if capability.resource_requirements.len() != realization.resources.len() {
                return Err(contract_mismatch(
                    &pool.pool_id,
                    "resource requirement shape drifted",
                ));
            }
            for (requirement, binding) in capability
                .resource_requirements
                .iter()
                .zip(&realization.resources)
            {
                let reserved = binding
                    .units
                    .checked_mul(u32::from(realization.member_capacity));
                let exact_offer = advertisement.resources.iter().filter(|offer| {
                    offer.pool_id == binding.pool_id && offer.class_id == binding.class_id
                });
                let offers = exact_offer.collect::<Vec<_>>();
                if requirement.protected_role.is_some()
                    || binding.protected.is_some()
                    || requirement.class_id != binding.class_id
                    || requirement.units != binding.units
                    || offers.len() != 1
                    || reserved.is_none_or(|units| offers[0].capacity_units < units)
                {
                    return Err(contract_mismatch(
                        &pool.pool_id,
                        "resource envelope drifted",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn valid_admission_grant(grant: &AuthorityGrant) -> bool {
    grant.contract_id.as_str() == SHARED_POOL_ADMIT_AUTHORITY_CONTRACT
        && grant.host_operation_contract_id.as_str() == SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT
        && grant.subject_kind.as_str() == SHARED_POOL_AUTHORITY_SUBJECT_KIND
        && !grant.host_id.as_str().is_empty()
        && !grant.boot_id.as_str().is_empty()
        && !grant.capability_id.as_str().is_empty()
}

fn contract_mismatch(
    pool_id: &conduit_core::SharedPoolId,
    detail: &str,
) -> (FailureReason, String) {
    (
        FailureReason::SharedPoolContractMismatch,
        format!("shared pool '{}': {detail}", pool_id.as_str()),
    )
}

#[cfg(test)]
mod tests {
    use super::validate_local_shared_pools;
    use conduit_core::{
        kind_id, port_id, ArtifactId, AuthorityContractId, AuthorityGrant, AuthorityGrantId,
        BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, FailureReason,
        HostAdvertisement, HostId, HostOperationContractId, HostProfileId, ImplementationId,
        KindContractRevision, OfferGeneration, PlacementId, PlannedSharedPool, PoolDeclarationId,
        PoolMemberLimits, PoolRealizationEnvelope, PortDescriptor, PortDirection, PortTemporal,
        SharedPoolId, PROTOCOL_VERSION, SHARED_POOL_ADMIT_AUTHORITY_CONTRACT,
        SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT, SHARED_POOL_AUTHORITY_SUBJECT_KIND,
    };

    fn capability() -> CapabilityOffer {
        CapabilityOffer {
            startup_parameters: Vec::new(),
            shorthand: None,
            capability_id: CapabilityId::from("member-capability"),
            kind_id: kind_id("chat/peer"),
            kind_contract_revision: KindContractRevision::from("chat/peer@1"),
            execution_profile_id: ExecutionProfileId::from("test/member@1"),
            implementation_id: ImplementationId::from("test/member@1"),
            artifact_id: ArtifactId::from("test/member-artifact@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("recv"),
                value_kind: kind_id("ChatMessage"),
                direction: PortDirection::Input,
                temporal: PortTemporal::Flow { closes: true },
            }],
            outputs: Vec::new(),
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: 2,
                max_queue_items: 2,
                max_queue_bytes: 512,
            },
        }
    }

    fn advertisement(capability: CapabilityOffer) -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("host"),
            boot_id: BootId::from("boot"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("test"),
            resources: Vec::new(),
            capabilities: vec![capability],
            planner_capabilities: Vec::new(),
        }
    }

    fn grant() -> AuthorityGrant {
        AuthorityGrant {
            grant_id: AuthorityGrantId::from("pool-admit"),
            contract_id: AuthorityContractId::from(SHARED_POOL_ADMIT_AUTHORITY_CONTRACT),
            host_operation_contract_id: HostOperationContractId::from(
                SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT,
            ),
            subject_kind: kind_id(SHARED_POOL_AUTHORITY_SUBJECT_KIND),
            host_id: HostId::from("host"),
            boot_id: BootId::from("boot"),
            capability_id: CapabilityId::from("member-capability"),
        }
    }

    fn pool(member: &CapabilityOffer) -> PlannedSharedPool {
        PlannedSharedPool {
            pool_id: SharedPoolId::from("room/peers"),
            declaration_id: PoolDeclarationId::from("room/peers/declaration"),
            member_face: member.checked_face(),
            maximum_members: 2,
            member_limits: PoolMemberLimits {
                queue_item_capacity: 2,
                queue_byte_capacity: 512,
                evidence_item_capacity: 8,
                evidence_byte_capacity: 1024,
            },
            realization_envelope: vec![PoolRealizationEnvelope {
                host_id: HostId::from("host"),
                boot_id: BootId::from("boot"),
                capability_id: CapabilityId::from("member-capability"),
                member_capacity: 2,
                resources: Vec::new(),
            }],
            admission_authority: AuthorityGrantId::from("pool-admit"),
            consumers: vec![PlacementId::from("room")],
        }
    }

    #[test]
    fn same_face_with_different_nominal_identity_remains_current() {
        let original = capability();
        let planned = pool(&original);
        let mut renamed = original;
        renamed.kind_id = kind_id("another/peer");
        renamed.kind_contract_revision = KindContractRevision::from("another/peer@9");
        let advertised = advertisement(renamed);
        assert_eq!(
            validate_local_shared_pools(&[planned], &advertised, &[grant()]),
            Ok(())
        );
    }

    #[test]
    fn face_limit_and_authority_drift_fail_distinctly() {
        let member = capability();
        let planned = pool(&member);
        let mut changed_face = member;
        changed_face.inputs[0].port_id = port_id("other");
        let advertised = advertisement(changed_face);
        assert!(matches!(
            validate_local_shared_pools(std::slice::from_ref(&planned), &advertised, &[grant()]),
            Err((FailureReason::SharedPoolContractMismatch, _))
        ));
        assert!(matches!(
            validate_local_shared_pools(&[planned], &advertised, &[]),
            Err((FailureReason::AuthorityContractMismatch, _))
        ));
    }
}
