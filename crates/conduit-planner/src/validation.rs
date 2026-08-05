use super::*;

pub(super) fn startup_order(
    placements: &[PlannedOperation],
    connections: &[PlannedConnection],
) -> Option<Vec<PlacementId>> {
    let mut remaining = placements
        .iter()
        .map(|placement| placement.placement_id.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let next = remaining
            .iter()
            .find(|candidate| {
                connections.iter().all(|connection| {
                    &connection.source_placement_id != *candidate
                        || !remaining.contains(&connection.sink_placement_id)
                })
            })
            .cloned()?;
        remaining.remove(&next);
        ordered.push(next);
    }
    Some(ordered)
}

pub(super) fn validate_operation_capability(
    operation: &CheckedOperation,
    capability: &conduit_core::CapabilityOffer,
) -> Result<(), PlannerError> {
    if capability.kind_id != operation.kind_id {
        return Err(PlannerError::WrongSemanticKind(format!(
            "operation '{}' requires '{}', capability '{}' offers '{}'",
            operation.operation_id.as_str(),
            operation.kind_id.as_str(),
            capability.capability_id.as_str(),
            capability.kind_id.as_str()
        )));
    }
    if capability.kind_contract_revision != operation.kind_contract_revision {
        return Err(PlannerError::WrongKindContractRevision(format!(
            "operation '{}' requires '{}', capability '{}' offers '{}'",
            operation.operation_id.as_str(),
            operation.kind_contract_revision.as_str(),
            capability.capability_id.as_str(),
            capability.kind_contract_revision.as_str()
        )));
    }
    if capability.inputs != operation.inputs || capability.outputs != operation.outputs {
        return Err(PlannerError::IncompatiblePortContract(format!(
            "operation '{}' ports differ from capability '{}'",
            operation.operation_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if capability.host_operations.iter().any(|requirement| {
        requirement.contract_id.as_str().is_empty()
            || requirement
                .target_kind
                .as_ref()
                .is_some_and(|target| target.as_str().is_empty())
            || requirement.maximum_in_flight == 0
    }) || capability
        .host_operations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidHostOperationRequirement(format!(
            "capability '{}' requirements must have non-empty identities, unique canonical ordering, and nonzero in-flight bounds",
            capability.capability_id.as_str()
        )));
    }
    if capability
        .resource_requirements
        .iter()
        .any(|requirement| requirement.class_id.as_str().is_empty() || requirement.units == 0)
        || capability
            .resource_requirements
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidResourceContract(format!(
            "capability '{}' requirements must have non-empty classes, positive units, and unique canonical ordering",
            capability.capability_id.as_str()
        )));
    }
    if capability.authority_requirements.iter().any(|requirement| {
        requirement.contract_id.as_str().is_empty()
            || requirement.host_operation_contract_id.as_str().is_empty()
            || requirement.subject_kind.as_str().is_empty()
            || !capability.host_operations.iter().any(|host_operation| {
                host_operation.contract_id == requirement.host_operation_contract_id
                    && host_operation.target_kind.as_ref() == Some(&requirement.subject_kind)
            })
    }) || capability
        .authority_requirements
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidAuthorityContract(format!(
            "capability '{}' authority requirements must bind a declared targeted host operation with non-empty identities and unique canonical ordering",
            capability.capability_id.as_str()
        )));
    }
    Ok(())
}

pub(super) fn validate_authority_grants(grants: &[AuthorityGrant]) -> Result<(), PlannerError> {
    if grants.iter().any(|grant| {
        grant.grant_id.as_str().is_empty()
            || grant.contract_id.as_str().is_empty()
            || grant.host_operation_contract_id.as_str().is_empty()
            || grant.subject_kind.as_str().is_empty()
            || grant.host_id.as_str().is_empty()
            || grant.boot_id.as_str().is_empty()
            || grant.capability_id.as_str().is_empty()
    }) {
        return Err(PlannerError::InvalidAuthorityContract(
            "grants must have non-empty immutable scope identities".to_string(),
        ));
    }
    let unique_ids = grants
        .iter()
        .map(|grant| &grant.grant_id)
        .collect::<BTreeSet<_>>();
    if unique_ids.len() != grants.len() {
        return Err(PlannerError::InvalidAuthorityContract(
            "grant identities must be unique".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_host_resources(host: &HostAdvertisement) -> Result<(), PlannerError> {
    if host.resources.iter().any(|resource| {
        resource.pool_id.as_str().is_empty()
            || resource.class_id.as_str().is_empty()
            || resource.capacity_units == 0
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
    Ok(())
}

pub(super) fn find_capability<'a>(
    realm: &'a [HostAdvertisement],
    host_id: &HostId,
    capability_id: &CapabilityId,
) -> Result<&'a conduit_core::CapabilityOffer, PlannerError> {
    realm
        .iter()
        .find(|host| &host.host_id == host_id)
        .and_then(|host| {
            host.capabilities
                .iter()
                .find(|item| &item.capability_id == capability_id)
        })
        .ok_or_else(|| PlannerError::UnknownCapability(capability_id.as_str().to_string()))
}

pub(super) fn select_provider(
    source: &PlannedOperation,
    sink: &PlannedOperation,
    providers: &[ConnectionProvider],
    requested: Option<ConnectionProvider>,
    link_bindings: &[LinkBinding],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<(ConnectionProvider, Option<LinkBinding>), PlannerError> {
    if source.host_id == sink.host_id {
        if requested.is_some_and(|provider| provider != ConnectionProvider::Local)
            || !providers.contains(&ConnectionProvider::Local)
        {
            return Err(PlannerError::UnavailableConnectionProvider(format!(
                "local provider unavailable for '{}' -> '{}'",
                source.operation_id.as_str(),
                sink.operation_id.as_str()
            )));
        }
        return Ok((ConnectionProvider::Local, None));
    }

    if requested == Some(ConnectionProvider::Local) {
        return Err(PlannerError::UnavailableConnectionProvider(format!(
            "local provider cannot connect '{}' -> '{}'",
            source.operation_id.as_str(),
            sink.operation_id.as_str()
        )));
    }
    let endpoint_matches = |binding: &&LinkBinding| {
        binding.source.host_id == source.host_id
            && binding.source.boot_id == source.boot_id
            && binding.sink.host_id == sink.host_id
            && binding.sink.boot_id == sink.boot_id
            && requested.is_none_or(|provider| binding.provider == provider)
    };
    let exact = link_bindings
        .iter()
        .filter(endpoint_matches)
        .collect::<Vec<_>>();
    if exact.is_empty() {
        return Err(PlannerError::LinkBindingMissing(format!(
            "no observed boot-scoped link for '{}' -> '{}'",
            source.operation_id.as_str(),
            sink.operation_id.as_str()
        )));
    }
    let ready = exact
        .into_iter()
        .filter(|binding| {
            binding.availability == LinkAvailability::Ready
                && binding.limits.maximum_in_flight_items >= connection_item_capacity
                && binding.limits.maximum_payload_bytes >= connection_byte_capacity
                && binding.limits.maximum_buffered_bytes >= connection_byte_capacity
                && binding.limits.maximum_frame_bytes >= binding.limits.maximum_payload_bytes
        })
        .collect::<Vec<_>>();
    if ready.is_empty() {
        return Err(PlannerError::LinkBindingUnavailable(format!(
            "observed link for '{}' -> '{}' is unavailable or below item/byte limits",
            source.operation_id.as_str(),
            sink.operation_id.as_str()
        )));
    }
    if ready.len() != 1 {
        return Err(PlannerError::LinkBindingAmbiguous(format!(
            "multiple observed links satisfy '{}' -> '{}'",
            source.operation_id.as_str(),
            sink.operation_id.as_str()
        )));
    }
    let binding = ready[0].clone();
    Ok((binding.provider, Some(binding)))
}

pub(super) fn validate_link_bindings(bindings: &[LinkBinding]) -> Result<(), PlannerError> {
    if bindings.iter().any(|binding| {
        binding.binding_id.as_str().is_empty()
            || binding.source.host_id.as_str().is_empty()
            || binding.source.boot_id.as_str().is_empty()
            || binding.source.endpoint_id.as_str().is_empty()
            || binding.sink.host_id.as_str().is_empty()
            || binding.sink.boot_id.as_str().is_empty()
            || binding.sink.endpoint_id.as_str().is_empty()
            || binding.source.endpoint_id == binding.sink.endpoint_id
            || binding.source.host_id == binding.sink.host_id
            || binding.provider == ConnectionProvider::Local
            || binding.provider_instance_id.as_str().is_empty()
            || binding.limits.maximum_in_flight_items == 0
            || binding.limits.maximum_payload_bytes == 0
            || binding.limits.maximum_buffered_bytes == 0
            || binding.limits.maximum_frame_bytes < binding.limits.maximum_payload_bytes
            || matches!(
                &binding.credential,
                conduit_core::LinkCredentialReference::Opaque(reference)
                    if reference.as_str().is_empty()
            )
            || matches!(
                &binding.authority,
                conduit_core::LinkAuthorityReference::Grant(grant_id)
                    if grant_id.as_str().is_empty()
            )
    }) {
        return Err(PlannerError::InvalidLinkBinding(
            "remote link bindings require non-empty distinct boot-scoped endpoints, one initialized non-local provider, and positive limits".to_string(),
        ));
    }
    let unique_ids = bindings
        .iter()
        .map(|binding| &binding.binding_id)
        .collect::<BTreeSet<_>>();
    if unique_ids.len() != bindings.len() {
        return Err(PlannerError::InvalidLinkBinding(
            "link binding identities must be unique".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn hash_string(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex(byte >> 4));
        encoded.push(hex(byte & 0x0f));
    }
    encoded
}

pub(super) fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!("nibble out of range"),
    }
}
