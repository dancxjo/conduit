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
