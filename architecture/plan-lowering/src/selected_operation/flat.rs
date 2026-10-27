//! Exact native binding over the immutable fully Core-verified flat projection.
use super::*;
use crate::local_tables::FlatDirectLocalKernelTables;
pub struct SelectedFlatDirectOperationPlan<
    'a,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
> {
    pub tables: &'a FlatDirectLocalKernelTables<'a, N, C, P, R, T, H>,
    pub active: &'a ActivePlayIdentity,
    pub placement_id: &'a str,
}
pub fn bind_selected_flat_direct_operation_for_subject<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContractRef<'_>,
    subject: &str,
    configuration: &[conduit_core::ConfigurationEntry],
    selection: SelectedFlatDirectOperationPlan<'_, N, C, P, R, T, H>,
) -> Result<NodeId, SelectedCallRefusal> {
    let view = selection.tables.preparation_scope().view();
    check_plan_scope_ref(
        claim,
        view.fragment_plan_id,
        view.header.host_id,
        view.header.boot_id,
        selection.active,
    )?;
    let mut matches = view
        .placements
        .iter()
        .filter(|gear| gear.placement_id == selection.placement_id);
    let gear = matches.next().ok_or(SelectedCallRefusal::WrongBinding)?;
    if matches.next().is_some() {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let gear = binding::checked_gear_ref(table, claim, &definition, subject, gear, |gear| {
        gear.configuration == configuration
    })?;
    if definition.kind.inputs.len() > 1 || definition.kind.outputs.len() > 1 {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    selection
        .tables
        .node_for_placement(gear.placement_id)
        .ok_or(SelectedCallRefusal::WrongBinding)
}
