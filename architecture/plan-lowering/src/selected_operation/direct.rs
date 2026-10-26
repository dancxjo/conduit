//! Bind native operation possession to an immutable direct numeric projection.
use super::*;
use crate::local_tables::DirectLocalKernelTables;

/// The direct table payload stays private and immutable through binding. Owned
/// extracted parts cannot provide this verified projection witness.
pub struct SelectedDirectOperationPlan<
    'a,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
> {
    pub tables: &'a DirectLocalKernelTables<'a, N, C, P, R, T, H>,
    pub active: &'a ActivePlayIdentity,
    pub placement_id: &'a PlacementId,
}
pub fn bind_selected_direct_operation<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    selection: SelectedDirectOperationPlan<'_, N, C, P, R, T, H>,
) -> Result<NodeId, SelectedCallRefusal> {
    bind_selected_direct_configured_operation(table, claim, definition, &[], selection)
}
pub fn bind_selected_direct_configured_operation<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    configuration: &[conduit_core::ConfigurationEntry],
    selection: SelectedDirectOperationPlan<'_, N, C, P, R, T, H>,
) -> Result<NodeId, SelectedCallRefusal> {
    let subject = &definition.kind.kind_id;
    bind_for_subject(table, claim, definition, subject, configuration, selection)
}
pub fn bind_selected_direct_operation_for_subject<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    subject: &conduit_core::KindId,
    selection: SelectedDirectOperationPlan<'_, N, C, P, R, T, H>,
) -> Result<NodeId, SelectedCallRefusal> {
    bind_for_subject(table, claim, definition, subject, &[], selection)
}
fn bind_for_subject<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    subject: &conduit_core::KindId,
    configuration: &[conduit_core::ConfigurationEntry],
    selection: SelectedDirectOperationPlan<'_, N, C, P, R, T, H>,
) -> Result<NodeId, SelectedCallRefusal> {
    let (plan, fragment) = selection.tables.preparation_scope();
    check_plan_scope(claim, fragment, selection.active)?;
    let view = plan
        .fragment(&fragment.fragment_id)
        .ok_or(SelectedCallRefusal::WrongBinding)?;
    let gear = binding::checked_gear(
        table,
        claim,
        &definition,
        subject,
        fragment,
        selection.placement_id,
        |gear| view.configuration_matches(&gear.placement_id, configuration),
    )?;
    if definition.kind.inputs.len() > 1 || definition.kind.outputs.len() > 1 {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    // Whole-fragment direct preparation fixed exact canonical placement order,
    // directional port numbering and the sole call/binding before sealing its
    // private numeric payload. The witness borrows those unmodified tables,
    // rather than accepting IDs or a caller-supplied LoweredPlanFragment.
    let node = selection
        .tables
        .node_for_placement(&gear.placement_id)
        .ok_or(SelectedCallRefusal::WrongBinding)?;
    Ok(node)
}
