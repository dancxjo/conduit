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
