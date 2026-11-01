//! Exact selection from immutable Core-verified metadata; possession stays local.
use super::*;
pub struct SelectedVerifiedOperationPlan<'a> {
    pub plan: &'a conduit_core::VerifiedPlanMetadata,
    pub fragment_id: &'a conduit_core::FragmentId,
    pub lowered: &'a LoweredPlanFragment,
    pub active: &'a ActivePlayIdentity,
    pub placement_id: &'a PlacementId,
}
pub fn bind_selected_verified_operation(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    selection: SelectedVerifiedOperationPlan<'_>,
) -> Result<NodeId, SelectedCallRefusal> {
    bind_selected_verified_configured_operation(table, claim, definition, &[], selection)
}
pub fn bind_selected_verified_configured_operation(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    configuration: &[conduit_core::ConfigurationEntry],
    selection: SelectedVerifiedOperationPlan<'_>,
) -> Result<NodeId, SelectedCallRefusal> {
    let subject = &definition.kind.kind_id;
    bind_for_subject(table, claim, definition, subject, configuration, selection)
}
pub fn bind_selected_verified_operation_for_subject(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    subject: &conduit_core::KindId,
    selection: SelectedVerifiedOperationPlan<'_>,
) -> Result<NodeId, SelectedCallRefusal> {
    bind_for_subject(table, claim, definition, subject, &[], selection)
}
fn bind_for_subject(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    subject: &conduit_core::KindId,
    configuration: &[conduit_core::ConfigurationEntry],
    selection: SelectedVerifiedOperationPlan<'_>,
) -> Result<NodeId, SelectedCallRefusal> {
    let view = selection
        .plan
        .fragment(selection.fragment_id)
        .ok_or(SelectedCallRefusal::WrongBinding)?;
    let ordinary = SelectedOperationPlan {
        fragment: view.lowering_metadata(),
        lowered: selection.lowered,
        active: selection.active,
        placement_id: selection.placement_id,
    };
    check_scope(claim, &ordinary)?;
    if crate::lowering::lower_verified_plan_metadata_fragment(selection.plan, selection.fragment_id)
        .map_err(|_| SelectedCallRefusal::WrongBinding)?
        != *selection.lowered
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    binding::bind_fields(table, claim, definition, subject, ordinary, |gear| {
        view.configuration_matches(&gear.placement_id, configuration)
    })
}

/// Independently retained local Fore pins for a Core-verified operation. This
/// establishes no Body route, availability, caller permit or native possession.
pub struct SelectedVerifiedForeOperationPlan<'a> {
    pub operation: SelectedVerifiedOperationPlan<'a>,
    pub fore: &'a [conduit_core::PlannedForePort],
}

/// Exact native identity and local Fore admission, in addition to the ordinary
/// verified configuration, lowering, grant and opaque possession checks.
pub fn bind_selected_verified_configured_fore_operation(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    configuration: &[conduit_core::ConfigurationEntry],
    native: crate::selected_assigned_operation::AssignedNativeIdentity<'_>,
    selection: SelectedVerifiedForeOperationPlan<'_>,
) -> Result<NodeId, SelectedCallRefusal> {
    let view = selection
        .operation
        .plan
        .fragment(selection.operation.fragment_id)
        .ok_or(SelectedCallRefusal::WrongBinding)?;
    check_fore_native(
        table,
        claim,
        &definition,
        native,
        view.lowering_metadata(),
        selection.fore,
        selection.operation.placement_id,
    )?;
    bind_selected_verified_configured_operation(
        table,
        claim,
        definition,
        configuration,
        selection.operation,
    )
}

fn check_fore_native(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: &SelectedOperationContract<'_>,
    native: crate::selected_assigned_operation::AssignedNativeIdentity<'_>,
    fragment: &PlanFragment,
    fore: &[conduit_core::PlannedForePort],
    placement: &PlacementId,
) -> Result<(), SelectedCallRefusal> {
    use conduit_core::BaseEnforcementClass;
    let wrong = || SelectedCallRefusal::WrongBinding;
    if fragment.fore_ports != fore
        || fore.is_empty()
        || fore.iter().any(|fore| fore.selected_line.is_some())
    {
        return Err(wrong());
    }
    let gear = fragment
        .placements
        .iter()
        .find(|gear| &gear.placement_id == placement)
        .ok_or_else(wrong)?;
    let base = gear.base.as_ref().ok_or_else(wrong)?;
    let scope = matching_scope(table, claim)?;
    let [resource] = gear.resources.as_slice() else {
        return Err(wrong());
    };
    if gear.capability_id.as_str() != native.capability
        || gear.execution_profile_id.as_str() != native.execution_profile
        || gear.artifact_id.as_str() != native.artifact
        || gear.limits != definition.kind.limits
        || base.base_id.as_str() != native.base
        || base.implementation_id.as_str() != native.base_implementation
        || base.mechanism_family.as_str() != native.mechanism
        || base.enforcement_class != BaseEnforcementClass::Cooperative
        || scope.authority_contract_id.as_str() != native.authority
        || resource.pool_id != claim.resource_pool_id
        || resource.class_id.as_str() != native.resource_class
        || resource.units != 1
    {
        return Err(wrong());
    }
    Ok(())
}
/// Direct selected operation: no caller-provided numeric projection enters.
pub struct SelectedProjectedForeOperationPlan<'a, 'p> {
    pub projection: &'a crate::lowering::VerifiedForeOperationProjection<'p>,
    pub active: &'a ActivePlayIdentity,
}
pub fn bind_selected_projected_configured_fore_operation(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    configuration: &[conduit_core::ConfigurationEntry],
    native: crate::selected_assigned_operation::AssignedNativeIdentity<'_>,
    selection: SelectedProjectedForeOperationPlan<'_, '_>,
) -> Result<NodeId, SelectedCallRefusal> {
    let projection = selection.projection;
    let fragment = projection.fragment;
    check_plan_scope(claim, fragment, selection.active)?;
    check_fore_native(
        table,
        claim,
        &definition,
        native,
        fragment,
        projection.fore(),
        &projection.node.placement_id,
    )?;
    let view = projection
        .plan
        .fragment(&fragment.fragment_id)
        .ok_or(SelectedCallRefusal::WrongBinding)?;
    let gear = binding::checked_gear(
        table,
        claim,
        &definition,
        &definition.kind.kind_id,
        fragment,
        &projection.node.placement_id,
        |gear| view.configuration_matches(&gear.placement_id, configuration),
    )?;
    binding::checked_numeric(gear, &projection.node, &projection.call, &definition)?;
    Ok(projection.node.node)
}
