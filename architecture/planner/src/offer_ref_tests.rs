//! Historical owned predicate versus both complete borrowed descriptor carriers.
use super::*;
fn historical_validate(
    gear: &CheckedGear,
    capability: &conduit_core::CapabilityOffer,
) -> Result<(), PlannerError> {
    if capability.kind_id != gear.kind_id {
        return Err(PlannerError::WrongSemanticKind(format!(
            "gear '{}' Kind differs from capability '{}' Kind",
            gear.gear_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if !gear.accepts_callable_front(capability) {
        return Err(PlannerError::IncompatibleCheckedFront(format!(
            "gear '{}' front differs from capability '{}' front",
            gear.gear_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if !gear.accepts_semantic_contract(capability) {
        return Err(PlannerError::WrongKindContractRevision(format!(
            "gear '{}' semantic laws or configuration contract differ from capability '{}'",
            gear.gear_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if gear.kind_contract_revision.as_str() != conduit_core::STRUCTURAL_POLYMORPHIC_CONTRACT
        && capability.kind_contract_revision != gear.kind_contract_revision
    {
        return Err(PlannerError::WrongKindContractRevision(format!(
            "gear '{}' semantic contract differs from capability '{}'",
            gear.gear_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if capability.host_calls.iter().any(|requirement| {
        requirement.contract_id.as_str().is_empty()
            || requirement
                .target_kind
                .as_ref()
                .is_some_and(|target| target.as_str().is_empty())
            || requirement.maximum_in_flight == 0
    }) || capability
        .host_calls
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidHostCallRequirement(format!(
            "capability '{}' requirements must have non-empty identities, unique canonical ordering, and nonzero in-flight bounds",
            capability.capability_id.as_str()
        )));
    }
    if capability.resource_requirements.iter().any(|requirement| {
        requirement.class_id.as_str().is_empty()
            || requirement.units == 0
            || requirement
                .compute
                .as_ref()
                .is_some_and(|compute| !compute.is_valid_for_units(requirement.units))
            || requirement
                .protected_role
                .as_ref()
                .is_some_and(|role| role.as_str().is_empty())
    }) || capability
        .resource_requirements
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidResourceContract(format!(
            "capability '{}' requirements must have non-empty classes and protected roles, positive units, and unique canonical ordering",
            capability.capability_id.as_str()
        )));
    }
    if capability.authority_requirements.iter().any(|requirement| {
        requirement.contract_id.as_str().is_empty()
            || requirement.host_call_contract_id.as_str().is_empty()
            || requirement.subject_kind.as_str().is_empty()
            || !capability.host_calls.iter().any(|host_call| {
                host_call.contract_id == requirement.host_call_contract_id
                    && host_call.target_kind.as_ref() == Some(&requirement.subject_kind)
            })
    }) || capability
        .authority_requirements
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidAuthorityContract(format!(
            "capability '{}' authority requirements must bind a declared targeted Host Call with non-empty identities and unique canonical ordering",
            capability.capability_id.as_str()
        )));
    }
    Ok(())
}

#[test]
fn operation_descriptors_preserve_complete_historical_refusals() {
    let plot = tests::plot();
    let host = tests::host();
    for gear in &plot.gears {
        for original in &host.capabilities {
            for mutation in 0..10 {
                let mut offer = original.clone();
                match mutation {
                    1 => offer.kind_id = "wrong-kind".into(),
                    2 => offer.kind_contract_revision = "wrong-revision".into(),
                    3 => offer.inputs.clear(),
                    4 => offer.outputs.clear(),
                    5 => {
                        if let Some(call) = offer.host_calls.first_mut() {
                            call.maximum_in_flight = 0;
                        }
                    }
                    6 => {
                        if let Some(call) = offer.host_calls.first().cloned() {
                            offer.host_calls.push(call);
                        }
                    }
                    7 => {
                        if let Some(resource) = offer.resource_requirements.first_mut() {
                            resource.units = 0;
                        }
                    }
                    8 => {
                        if let Some(resource) = offer.resource_requirements.first().cloned() {
                            offer.resource_requirements.push(resource);
                        }
                    }
                    9 => offer
                        .authority_requirements
                        .push(conduit_core::AuthorityRequirement {
                            contract_id: "authority".into(),
                            host_call_contract_id: "missing".into(),
                            subject_kind: "subject".into(),
                        }),
                    _ => {}
                }
                let inputs = offer.inputs.iter().map(Into::into).collect::<Vec<_>>();
                let outputs = offer.outputs.iter().map(Into::into).collect::<Vec<_>>();
                let calls = offer.host_calls.iter().map(Into::into).collect::<Vec<_>>();
                let resources = offer
                    .resource_requirements
                    .iter()
                    .map(Into::into)
                    .collect::<Vec<_>>();
                let authorities = offer
                    .authority_requirements
                    .iter()
                    .map(Into::into)
                    .collect::<Vec<_>>();
                let mut borrowed = conduit_core::CapabilityOfferRef::from(&offer);
                borrowed.inputs = conduit_core::PortDescriptorRefs::Borrowed(&inputs);
                borrowed.outputs = conduit_core::PortDescriptorRefs::Borrowed(&outputs);
                borrowed.host_calls = conduit_core::HostCallRequirementRefs::Borrowed(&calls);
                borrowed.resource_requirements =
                    conduit_core::ResourceRequirementRefs::Borrowed(&resources);
                borrowed.authority_requirements =
                    conduit_core::AuthorityRequirementRefs::Borrowed(&authorities);
                let expected = historical_validate(gear, &offer);
                assert_eq!(validate_operation_capability(gear, &offer), expected);
                assert_eq!(
                    validate_operation_capability_ref(gear.into(), borrowed),
                    expected
                );
            }
        }
    }
}

fn historical_validate_host_resources(host: &HostAdvertisement) -> Result<(), PlannerError> {
    if host.resources.iter().any(|resource| {
        resource.pool_id.as_str().is_empty()
            || resource.class_id.as_str().is_empty()
            || resource.capacity_units == 0
            || resource
                .compute
                .as_ref()
                .is_some_and(|compute| !compute.is_valid_for_capacity(resource.capacity_units))
    }) || host
        .resources
        .windows(2)
        .any(|pair| pair[0].pool_id >= pair[1].pool_id)
    {
        return Err(PlannerError::InvalidResourceContract(format!(
            "host '{}' pools must have non-empty identities, positive capacity, and unique pool-id ordering",
            host.host_id.as_str()
        )));
    }
    for base in &host.bases {
        if base.lifecycle != conduit_core::BaseLifecycle::Ready
            || base.base_id.as_str().is_empty()
            || base.provider_instance_id.as_str().is_empty()
            || base.provider_generation == 0
            || base.implementation_id.as_str().is_empty()
            || base.mechanism_family.as_str().is_empty()
            || base.capability_ids.is_empty() && base.resource_pool_ids.is_empty()
            || base.capability_ids.iter().any(|id| {
                !host
                    .capabilities
                    .iter()
                    .any(|offer| offer.capability_id == *id)
            })
            || base
                .resource_pool_ids
                .iter()
                .any(|id| !host.resources.iter().any(|offer| offer.pool_id == *id))
        {
            return Err(PlannerError::InvalidResourceContract(format!(
                "host '{}' has invalid current Base provenance",
                host.host_id.as_str()
            )));
        }
    }
    Ok(())
}

#[test]
fn current_host_description_preserves_historical_provenance_refusals() {
    use conduit_core as c;
    for mutation in 0..13 {
        let mut host = tests::host();
        host.resources = vec![c::ResourceOffer {
            pool_id: "pool".into(),
            class_id: "class".into(),
            capacity_units: 1,
            compute: None,
            content: None,
        }];
        host.bases = vec![c::BaseProviderAdvertisement {
            base_id: "base".into(),
            provider_instance_id: "instance".into(),
            provider_generation: 1,
            implementation_id: "implementation".into(),
            mechanism_family: "family".into(),
            enforcement_class: c::BaseEnforcementClass::Cooperative,
            lifecycle: c::BaseLifecycle::Ready,
            capability_ids: host
                .capabilities
                .iter()
                .map(|v| v.capability_id.clone())
                .collect(),
            resource_pool_ids: vec!["pool".into()],
        }];
        match mutation {
            1 => host.resources[0].pool_id = "".into(),
            2 => host.resources[0].class_id = "".into(),
            3 => host.resources[0].capacity_units = 0,
            4 => host.resources.push(host.resources[0].clone()),
            5 => host.bases[0].lifecycle = c::BaseLifecycle::Lost,
            6 => host.bases[0].provider_generation = 0,
            7 => host.bases[0].base_id = "".into(),
            8 => host.bases[0].provider_instance_id = "".into(),
            9 => host.bases[0].implementation_id = "".into(),
            10 => host.bases[0].mechanism_family = "".into(),
            11 => host.bases[0].capability_ids.push("absent".into()),
            12 => {
                host.resources[0].capacity_units = 0;
                host.bases[0].provider_generation = 0;
            }
            _ => {}
        }
        let resources: Vec<_> = host.resources.iter().map(Into::into).collect();
        let capabilities: Vec<_> = host.capabilities.iter().map(Into::into).collect();
        let ids: Vec<_> = host.bases[0]
            .capability_ids
            .iter()
            .map(|v| v.as_str())
            .collect();
        let pools: Vec<_> = host.bases[0]
            .resource_pool_ids
            .iter()
            .map(|v| v.as_str())
            .collect();
        let bases = [c::BaseProviderAdvertisementRef {
            capability_ids: c::CapabilityIdRefs::Borrowed(&ids),
            resource_pool_ids: c::ResourcePoolIdRefs::Borrowed(&pools),
            ..(&host.bases[0]).into()
        }];
        let borrowed = c::HostAdvertisementRef {
            resources: c::ResourceOfferRefs::Borrowed(&resources),
            capabilities: c::CapabilityOfferRefs::Borrowed(&capabilities),
            bases: c::BaseProviderAdvertisementRefs::Borrowed(&bases),
            ..(&host).into()
        };
        let expected = historical_validate_host_resources(&host);
        assert_eq!(
            validate_host_resources(&host),
            expected,
            "owned mutation {mutation}"
        );
        assert_eq!(
            validate_host_resources_ref(borrowed),
            expected,
            "borrowed mutation {mutation}"
        );
    }
}
