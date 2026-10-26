//! Shared preparation checks for exact admitted single-call machine Backs.
use crate::lowering::LoweredPlanFragment;
use conduit_core::{
    ActivePlayIdentity, BaseCapabilityTable, BaseOperationClaim, Kind, PlacementId, PlanFragment,
};
use conduit_kernel::{HostCallId, NodeId, PortId};
mod binding;
mod verified;
pub use verified::{
    bind_selected_verified_configured_fore_operation, bind_selected_verified_configured_operation,
    bind_selected_verified_operation, bind_selected_verified_operation_for_subject,
    SelectedVerifiedForeOperationPlan, SelectedVerifiedOperationPlan,
};

pub struct SelectedOperationContract<'a> {
    pub kind: &'a Kind,
    pub call: &'a str,
    pub input_bytes: u32,
    pub output_bytes: u32,
    pub resource_bytes: u64,
}

pub struct SelectedOperationPlan<'a> {
    pub fragment: &'a PlanFragment,
    pub lowered: &'a LoweredPlanFragment,
    pub active: &'a ActivePlayIdentity,
    pub placement_id: &'a PlacementId,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SelectedCallRefusal {
    WrongBinding,
    Possession,
}

pub fn bind_selected_operation(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    selection: SelectedOperationPlan<'_>,
) -> Result<NodeId, SelectedCallRefusal> {
    bind_selected_configured_operation(table, claim, definition, &[], selection)
}
/// Bind against independently retained checked Source configuration. Incoming
/// Plan entries cannot provide their own expected configuration.
pub fn bind_selected_configured_operation(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    configuration: &[conduit_core::ConfigurationEntry],
    selection: SelectedOperationPlan<'_>,
) -> Result<NodeId, SelectedCallRefusal> {
    let subject = &definition.kind.kind_id;
    bind_configured_for_subject(table, claim, definition, subject, configuration, selection)
}
/// The initialized native implementation supplies the independent authority
/// subject. Incoming claims and Plans cannot select this expectation.
pub fn bind_selected_operation_for_subject(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    subject: &conduit_core::KindId,
    selection: SelectedOperationPlan<'_>,
) -> Result<NodeId, SelectedCallRefusal> {
    bind_configured_for_subject(table, claim, definition, subject, &[], selection)
}
fn bind_configured_for_subject(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    subject: &conduit_core::KindId,
    configuration: &[conduit_core::ConfigurationEntry],
    selection: SelectedOperationPlan<'_>,
) -> Result<NodeId, SelectedCallRefusal> {
    if !conduit_core::verify_plan_fragment(selection.fragment) {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    check_scope(claim, &selection)?;
    // Matching IDs cannot replace exact ordinary numeric lowering.
    if crate::lowering::lower_plan_fragment(selection.fragment)
        .map_err(|_| SelectedCallRefusal::WrongBinding)?
        != *selection.lowered
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    binding::bind_fields(table, claim, definition, subject, selection, |gear| {
        gear.configuration == configuration
    })
}
fn check_scope(
    claim: &BaseOperationClaim,
    selection: &SelectedOperationPlan<'_>,
) -> Result<(), SelectedCallRefusal> {
    let fragment = selection.fragment;
    let lowered = selection.lowered;
    let active = selection.active;
    if fragment.plan_id != claim.plan_id
        || fragment.host_id != claim.host_id
        || fragment.boot_id != claim.boot_id
        || active.plan_id != claim.plan_id
        || active.host_id != claim.host_id
        || active.boot_id != claim.boot_id
        || active.active_play_id != claim.active_play_id
        || conduit_core::bind_active_play(
            &claim.plan_id,
            &claim.host_id,
            &claim.boot_id,
            active.play_sequence,
        ) != *active
        || lowered.identity.plan_id != fragment.plan_id
        || lowered.identity.fragment_id != fragment.fragment_id
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    Ok(())
}

/// Shared current possession check for rich and constrained assigned projections.
pub(crate) fn matching_scope<'a>(
    table: &'a BaseCapabilityTable,
    claim: &BaseOperationClaim,
) -> Result<&'a conduit_core::BaseCapabilityScope, SelectedCallRefusal> {
    let inspection = table
        .inspections()
        .next()
        .ok_or(SelectedCallRefusal::Possession)?;
    if inspection.lifecycle != conduit_core::CapabilityLifecycle::Issued
        || inspection.in_flight_operations != 0
    {
        return Err(SelectedCallRefusal::Possession);
    }
    let scope = &inspection.scope;
    if scope.host_id != claim.host_id
        || scope.boot_id != claim.boot_id
        || scope.plan_id != claim.plan_id
        || scope.active_play_id != claim.active_play_id
        || scope.base_instance_id != claim.base_instance_id
        || scope.base_provider_generation != claim.base_provider_generation
        || scope.implementation_id != claim.implementation_id
        || scope.operation_contract_id != claim.operation_contract_id
        || scope.subject_kind != claim.subject_kind
        || scope.resource_pool_id != claim.resource_pool_id
        || scope.resource_generation_id != claim.resource_generation_id
        || scope.envelope_id != claim.envelope_id
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    Ok(scope)
}
