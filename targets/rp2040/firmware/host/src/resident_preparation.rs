//! Root composition over real initialized Backs; no catalog decode or transfer.
use conduit_body::{
    check_host_preparation_delegation, decode_body_fragment_document,
    receive_delegated_body_fragment_with_approved_fore, AssignedBodyPreparationProfile,
    AttributedHostCommand, HostPreparationDelegation, NativePreparationContext,
};
use conduit_core as c;
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_expression::SelectedAssignedExpressionPlan,
    selected_assigned_operation::AssignedOperationInput,
};

#[derive(Debug, PartialEq, Eq)]
pub enum ValidationRefusal {
    Unavailable,
    Policy,
    Document,
    Selection,
    Bounds,
    Kernel,
    Assignment,
}

pub fn validate(
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'_, '_>,
) -> Result<(), ValidationRefusal> {
    let owner = crate::resident::inventory().map_err(|_| ValidationRefusal::Unavailable)?;
    validate_with_inventory(
        &owner,
        installed,
        context,
        command,
        crate::config::RESIDENT_PREPARATION_MAXIMA,
    )
}

/// Check the complete fragment against actual initialized entries, without
/// reserving resources, withdrawing a provider or claiming a prepared kernel.
pub fn validate_with_inventory(
    owner: &crate::resident::generated::Initialized,
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'_, '_>,
    maxima: c::AssignedPlanMaxima,
) -> Result<(), ValidationRefusal> {
    use ValidationRefusal as E;
    check_host_preparation_delegation(installed, context, &command).map_err(|_| E::Policy)?;
    let (_, projection) =
        decode_body_fragment_document(command.payload()).map_err(|_| E::Document)?;
    let selected = owner
        .identify_assignments(projection)
        .map_err(|_| E::Selection)?;
    let pins = selected.computations();
    let maximum_text_bytes = pins
        .iter()
        .map(|p| p.text_bytes)
        .max()
        .ok_or(E::Selection)?;
    let maximum_total_text_bytes = pins
        .iter()
        .try_fold(0u32, |sum, p| sum.checked_add(p.text_bytes))
        .ok_or(E::Bounds)?;
    let requirements = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text(context.actual.host_id.as_str()),
        boot: c::AssignedIdentity::from_text(context.actual.boot_id.as_str()),
        host_calls: &[],
        resources: &[0],
        remote_bindings: &[],
    };
    let computations = c::AssignedComputationProfile {
        maximum_text_bytes,
        maximum_total_text_bytes,
        requirements: pins,
    };
    let (fragment, fore) = receive_delegated_body_fragment_with_approved_fore::<16>(
        installed,
        context,
        command,
        AssignedBodyPreparationProfile {
            maxima,
            requirements,
            computations,
            fore: None,
        },
    )
    .map_err(|_| E::Assignment)?;
    // Prepare inside the installed fixed capacities using the same active-prefix
    // scheduler topology. This does not admit its value/sign/queue workspace.
    conduit_plan_lowering::delegated_kernel::prepare_delegated_body_kernel_tables::<
        128,
        { crate::resident::generated::ENTRY_COUNT },
        { crate::resident::generated::ENTRY_COUNT * 2 },
        1,
        { crate::resident::generated::ENTRY_COUNT },
        { crate::resident::generated::ENTRY_COUNT * 2 },
        0,
    >(
        &fragment,
        AssignedBodyPreparationProfile {
            maxima,
            requirements,
            computations,
            fore: Some(fore.as_slice()),
        },
    )
    .map_err(|_| E::Kernel)?;
    for choice in selected.nodes() {
        owner
            .check_entry(
                choice.entry,
                SelectedAssignedExpressionPlan {
                    input: AssignedOperationInput::Contiguous(fragment.projection()),
                    maxima,
                    requirements,
                    computations,
                    fore: Some(fore.as_slice()),
                    fragment_id: fragment.metadata().fragment,
                    scope: AssignedPlayScope::Delegated(&fragment),
                    node: conduit_kernel::NodeId(choice.node),
                    work_resource: 0,
                },
            )
            .map_err(|_| E::Selection)?;
    }
    Ok(())
}
