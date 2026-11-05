//! Root composition over real initialized Backs; no catalog decode or transfer.
use conduit_body::{
    check_host_preparation_delegation, decode_body_fragment_document,
    receive_delegated_body_fragment_with_approved_fore, receive_delegated_body_workset,
    ApprovedBodyForePorts, AssignedBodyPreparationProfile, AttributedHostCommand,
    DelegatedBodyFragment, HostPreparationDelegation, NativePreparationContext,
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
    Workset,
}

pub fn validate(
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'_, '_>,
) -> Result<(), ValidationRefusal> {
    let owner = crate::resident::inventory().map_err(|_| ValidationRefusal::Unavailable)?;
    validate_workset_with_inventory(
        &owner,
        installed,
        context,
        command,
        crate::config::RESIDENT_PREPARATION_MAXIMA,
    )
}

/// The original signed command must contain the complete local set before any
/// fragment profile is prepared. No resources/providers are changed here.
pub fn validate_workset_with_inventory(
    owner: &crate::resident::generated::Initialized,
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'_, '_>,
    maxima: c::AssignedPlanMaxima,
) -> Result<(), ValidationRefusal> {
    let workset = receive_delegated_body_workset::<{ crate::resident::generated::ENTRY_COUNT }>(
        installed, context, command,
    )
    .map_err(|_| ValidationRefusal::Workset)?;
    if workset.declared_runtime_state_bytes() > u32::from(maxima.runtime_state_bytes) {
        return Err(ValidationRefusal::Bounds);
    }
    let mut used = [false; crate::resident::generated::ENTRY_COUNT];
    for index in 0..workset.fragment_count() {
        let document = workset.document(index).ok_or(ValidationRefusal::Document)?;
        let selections = validate_document(owner, context, document, maxima, |profile| {
            workset
                .receive_fragment_with_approved_fore::<16>(index, installed, context, profile)
                .map_err(|_| ValidationRefusal::Assignment)
        })?;
        for (prior, new) in used.iter_mut().zip(selections) {
            if *prior && new {
                return Err(ValidationRefusal::Selection);
            }
            *prior |= new;
        }
    }
    Ok(())
}

/// Check the complete fragment against actual initialized entries, without
/// reserving resources, withdrawing a provider or claiming a prepared kernel.
#[allow(dead_code)] // Independent single-fragment component qualification, not a wire entrance.
pub fn validate_with_inventory(
    owner: &crate::resident::generated::Initialized,
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'_, '_>,
    maxima: c::AssignedPlanMaxima,
) -> Result<(), ValidationRefusal> {
    use ValidationRefusal as E;
    check_host_preparation_delegation(installed, context, &command).map_err(|_| E::Policy)?;
    validate_document(owner, context, command.payload(), maxima, |profile| {
        receive_delegated_body_fragment_with_approved_fore::<16>(
            installed, context, command, profile,
        )
        .map_err(|_| E::Assignment)
    })
    .map(|_| ())
}

fn validate_document<'a>(
    owner: &crate::resident::generated::Initialized,
    context: NativePreparationContext<'_>,
    document: &'a [u8],
    maxima: c::AssignedPlanMaxima,
    approve: impl FnOnce(
        AssignedBodyPreparationProfile<'_>,
    ) -> Result<
        (DelegatedBodyFragment<'a>, ApprovedBodyForePorts<16>),
        ValidationRefusal,
    >,
) -> Result<[bool; crate::resident::generated::ENTRY_COUNT], ValidationRefusal> {
    use ValidationRefusal as E;
    let (_, projection) = decode_body_fragment_document(document).map_err(|_| E::Document)?;
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
    let (fragment, fore) = approve(AssignedBodyPreparationProfile {
        maxima,
        requirements,
        computations,
        fore: None,
    })?;
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
    let mut used = [false; crate::resident::generated::ENTRY_COUNT];
    for choice in selected.nodes() {
        used[choice.entry] = true;
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
    Ok(used)
}
