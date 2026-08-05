use super::*;

pub(super) fn validate_fragment_execution_contract(
    fragment: &PlanFragment,
) -> Option<(FailureReason, String)> {
    if fragment.placements.iter().any(|placement| {
        placement.host_operations.iter().any(|requirement| {
            requirement.contract_id.as_str().is_empty()
                || requirement
                    .target_kind
                    .as_ref()
                    .is_some_and(|target| target.as_str().is_empty())
                || requirement.maximum_in_flight == 0
        }) || placement
            .host_operations
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    }) {
        return Some((
            FailureReason::HostOperationContractMismatch,
            "host-operation requirements must have non-empty identities, unique canonical ordering, and nonzero in-flight bounds".to_string(),
        ));
    }
    if fragment.connections.iter().any(|connection| {
        let invalid_binding = connection.link_binding.as_ref().is_some_and(|binding| {
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
                || binding.availability != conduit_core::LinkAvailability::Ready
                || binding.limits.maximum_in_flight_items < connection.item_capacity
                || binding.limits.maximum_payload_bytes < connection.byte_capacity
                || binding.limits.maximum_buffered_bytes < connection.byte_capacity
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
        });
        invalid_binding
            || match connection.provider {
                ConnectionProvider::Local => connection.link_binding.is_some(),
                ConnectionProvider::InMemory
                | ConnectionProvider::FixtureFrame
                | ConnectionProvider::FixtureDatagram
                | ConnectionProvider::WebSocket => connection
                    .link_binding
                    .as_ref()
                    .is_none_or(|binding| binding.provider != connection.provider),
            }
    }) {
        return Some((
            FailureReason::LinkBindingMismatch,
            "remote connections require one ready exact non-local link binding with initialized provider, explicit credential/authority references, and sufficient limits; local connections must not bind a link".to_string(),
        ));
    }
    if fragment.placements.iter().any(|placement| {
        placement.authority.iter().any(|binding| {
            binding.grant_id.as_str().is_empty()
                || binding.contract_id.as_str().is_empty()
                || binding.host_operation_contract_id.as_str().is_empty()
                || binding.subject_kind.as_str().is_empty()
                || binding.host_id.as_str().is_empty()
                || binding.boot_id.as_str().is_empty()
                || binding.capability_id.as_str().is_empty()
        }) || placement
            .authority
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    }) {
        return Some((
            FailureReason::AuthorityContractMismatch,
            "authority bindings must have non-empty exact scope identities and unique canonical ordering".to_string(),
        ));
    }
    if fragment.placements.iter().any(|placement| {
        placement.resources.iter().any(|binding| {
            binding.pool_id.as_str().is_empty()
                || binding.class_id.as_str().is_empty()
                || binding.units == 0
        }) || placement
            .resources
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    }) {
        return Some((
            FailureReason::ResourceContractMismatch,
            "resource bindings must have non-empty identities, positive units, and unique canonical ordering".to_string(),
        ));
    }
    if fragment.cancellation_policy != CancellationPolicy::CancelAllAndRejectLateCompletion {
        return Some((
            FailureReason::UnsupportedCancellationPolicy,
            "host supports only cancel-all with late-completion rejection".to_string(),
        ));
    }
    if fragment.terminal_policy != TerminalPolicy::RequireAllPlacementsAndConnections {
        return Some((
            FailureReason::UnsupportedTerminalPolicy,
            "host requires terminal evidence for every placement and connection".to_string(),
        ));
    }

    let expected_dependencies = fragment
        .connections
        .iter()
        .map(|connection| StartupDependency {
            prerequisite_placement_id: connection.sink_placement_id.clone(),
            dependent_placement_id: connection.source_placement_id.clone(),
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if fragment.startup_dependencies != expected_dependencies {
        return Some((
            FailureReason::InvalidStartupDependencies,
            "startup dependencies do not match the exact cord endpoints".to_string(),
        ));
    }

    let local_placements = fragment
        .placements
        .iter()
        .map(|placement| placement.placement_id.clone())
        .collect::<BTreeSet<_>>();
    let ordered_placements = fragment
        .startup_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if ordered_placements != local_placements
        || fragment.startup_order.len() != local_placements.len()
    {
        return Some((
            FailureReason::InvalidStartupDependencies,
            "startup order must name every local placement exactly once".to_string(),
        ));
    }
    let positions = fragment
        .startup_order
        .iter()
        .enumerate()
        .map(|(index, placement_id)| (placement_id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    if fragment.startup_dependencies.iter().any(|dependency| {
        let prerequisite = positions.get(&dependency.prerequisite_placement_id);
        let dependent = positions.get(&dependency.dependent_placement_id);
        matches!((prerequisite, dependent), (Some(before), Some(after)) if before >= after)
    }) {
        return Some((
            FailureReason::InvalidStartupDependencies,
            "startup order violates a local prerequisite".to_string(),
        ));
    }

    let expected_terminals = fragment
        .placements
        .iter()
        .map(|placement| ExpectedTerminal::PlacementCompleted(placement.placement_id.clone()))
        .chain(fragment.connections.iter().map(|connection| {
            ExpectedTerminal::ConnectionCompleted(connection.connection_id.clone())
        }))
        .chain(core::iter::once(ExpectedTerminal::PlanCompleted))
        .collect::<Vec<_>>();
    if fragment.expected_terminals != expected_terminals {
        return Some((
            FailureReason::UnsupportedTerminalPolicy,
            "terminal requirements do not cover every planned placement and connection".to_string(),
        ));
    }

    let expected_evidence =
        core::iter::once(ExpectedEvidence::PlanFragmentReceived)
            .chain(fragment.placements.iter().map(|placement| {
                ExpectedEvidence::PlacementPrepared(placement.placement_id.clone())
            }))
            .chain(fragment.placements.iter().map(|placement| {
                ExpectedEvidence::PlacementTerminal(placement.placement_id.clone())
            }))
            .chain(fragment.connections.iter().map(|connection| {
                ExpectedEvidence::ConnectionTerminal(connection.connection_id.clone())
            }))
            .chain(core::iter::once(ExpectedEvidence::PlanTerminal))
            .collect::<Vec<_>>();
    if fragment.expected_evidence != expected_evidence {
        return Some((
            FailureReason::EvidenceBudgetExceeded,
            "mandatory evidence descriptors do not cover the exact fragment".to_string(),
        ));
    }
    let Some(required) = mandatory_evidence_storage_requirement(&fragment.expected_evidence) else {
        return Some((
            FailureReason::EvidenceBudgetExceeded,
            "mandatory evidence cannot be represented by the public budget types".to_string(),
        ));
    };
    if fragment.evidence_storage_budget.item_capacity < required.item_capacity
        || fragment.evidence_storage_budget.byte_capacity < required.byte_capacity
    {
        return Some((
            FailureReason::EvidenceBudgetExceeded,
            "mandatory evidence exceeds its planned item or byte budget".to_string(),
        ));
    }
    None
}

pub(super) fn validate_host_operation_action(
    placement: &PlannedOperation,
    action: &OperationAction,
) -> Result<(), ImplementationFailure> {
    let (contract, target_kind, input_bytes) = match action {
        OperationAction::Wait { .. } => (
            conduit_core::WAIT_HOST_OPERATION_CONTRACT,
            None,
            core::mem::size_of::<u64>() as u32,
        ),
        OperationAction::Present {
            presentation_kind,
            value,
        } => (
            conduit_core::PRESENT_HOST_OPERATION_CONTRACT,
            Some(presentation_kind),
            value.encoded_len(),
        ),
        _ => return Ok(()),
    };
    let Some(requirement) = placement.host_operations.iter().find(|requirement| {
        requirement.contract_id.as_str() == contract
            && requirement.target_kind.as_ref() == target_kind
    }) else {
        return Err(ImplementationFailure::new(
            FailureReason::HostOperationNotPlanned,
            format!(
                "placement '{}' requested unplanned host operation '{}'",
                placement.placement_id.as_str(),
                contract
            ),
        ));
    };
    if requirement.maximum_in_flight == 0 || input_bytes > requirement.maximum_input_bytes {
        return Err(ImplementationFailure::new(
            FailureReason::HostOperationInputExceeded,
            format!(
                "placement '{}' host operation '{}' input requires {} bytes above bound {}",
                placement.placement_id.as_str(),
                contract,
                input_bytes,
                requirement.maximum_input_bytes
            ),
        ));
    }
    Ok(())
}

pub(super) fn validate_authority_action(
    placement: &PlannedOperation,
    action: &OperationAction,
) -> Result<(), ImplementationFailure> {
    let (contract, target_kind) = match action {
        OperationAction::Wait { .. } => (conduit_core::WAIT_HOST_OPERATION_CONTRACT, None),
        OperationAction::Present {
            presentation_kind, ..
        } => (
            conduit_core::PRESENT_HOST_OPERATION_CONTRACT,
            Some(presentation_kind),
        ),
        _ => return Ok(()),
    };
    let requires_authority = placement
        .authority
        .iter()
        .any(|binding| binding.host_operation_contract_id.as_str() == contract);
    if requires_authority
        && !placement.authority.iter().any(|binding| {
            binding.host_operation_contract_id.as_str() == contract
                && Some(&binding.subject_kind) == target_kind
        })
    {
        return Err(ImplementationFailure::new(
            FailureReason::AuthorityDenied,
            format!(
                "placement '{}' lacks authority for host operation '{}' and requested subject",
                placement.placement_id.as_str(),
                contract
            ),
        ));
    }
    Ok(())
}

pub(super) fn authority_binding_matches_current_grant(
    binding: &conduit_core::AuthorityBinding,
    grants: &[conduit_core::AuthorityGrant],
) -> bool {
    let mut matches = grants
        .iter()
        .filter(|grant| grant.grant_id == binding.grant_id);
    matches.next().is_some_and(|grant| {
        grant.contract_id == binding.contract_id
            && grant.host_operation_contract_id == binding.host_operation_contract_id
            && grant.subject_kind == binding.subject_kind
            && grant.host_id == binding.host_id
            && grant.boot_id == binding.boot_id
            && grant.capability_id == binding.capability_id
    }) && matches.next().is_none()
}

pub(super) fn link_binding_matches_current_observation(
    binding: &conduit_core::LinkBinding,
    observations: &[conduit_core::LinkBinding],
) -> bool {
    let mut matches = observations
        .iter()
        .filter(|observation| observation.binding_id == binding.binding_id);
    matches
        .next()
        .is_some_and(|observation| observation == binding)
        && matches.next().is_none()
}
