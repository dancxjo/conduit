//! Root composition over real initialized Backs; no catalog decode or transfer.
#[path = "resident_execution.rs"]
mod execution;
pub use execution::{retire_workset, step_workset, TerminalFragment};
#[path = "resident_resources.rs"]
mod resources;
#[path = "resident_storage.rs"]
mod storage;
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
pub use resources::observe_private_work;
pub use storage::{
    AttemptStorage, BoundFragment, BoundKernel, BoundWorkset, CommittedWorkset, RetiredKernel,
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
    Retention(conduit_body::HostCommandRetentionRefusal),
    Execution(conduit_body::ExecutionDelegationRefusal),
    Storage(conduit_kernel::scheduler::KernelPreparationError),
}

pub use crate::resident::inventory;

/// Independently installed policy and actual Root occupancy, not wire fields.
#[derive(Clone, Copy)]
pub struct ExecutionPolicy<'a> {
    pub installed: Option<&'a conduit_body::HostExecutionDelegation>,
    pub active_worksets: u16,
}

/// Apply execution policy to the original retained workset before allocating
/// kernels or lending Backs. Successful preparation still requires atomic
/// resource admission, native possession where applicable and current Start.
pub fn prepare_retained_execution_storage<'a, 'o, const N: usize>(
    owner: &'o mut crate::resident::generated::Initialized,
    installed: Option<&HostPreparationDelegation>,
    execution: ExecutionPolicy<'_>,
    context: NativePreparationContext<'_>,
    workspace: &'a conduit_body::HostCommandInput<N>,
    maxima: c::AssignedPlanMaxima,
    maximum_storage_bytes: usize,
) -> Result<AttemptStorage<'a, 'o>, ValidationRefusal> {
    let command = workspace
        .reborrow(
            context.member,
            context.actual,
            &context.bootstrap.owner,
            conduit_body::HOST_PREPARED_WORKSET_CONTRACT,
            context.now_millis,
        )
        .map_err(ValidationRefusal::Retention)?;
    let checked = check_workset(owner, installed, context, command, maxima, Some(execution))?;
    storage::prepare(
        owner,
        checked.documents,
        checked.stamp,
        maximum_storage_bytes,
    )
}

/// Prepare the original sealed input after its delivery future has ended.
/// Recheck current membership, Boot, issuer and deadline before decoding; RAM
/// custody never renews attribution or grants permission to Start. The returned
/// storage borrows this same immutable workspace and the actual initialized Backs.
pub fn prepare_retained_workset_storage<'a, 'o, const N: usize>(
    owner: &'o mut crate::resident::generated::Initialized,
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    workspace: &'a conduit_body::HostCommandInput<N>,
    maxima: c::AssignedPlanMaxima,
    maximum_storage_bytes: usize,
) -> Result<AttemptStorage<'a, 'o>, ValidationRefusal> {
    let command = workspace
        .reborrow(
            context.member,
            context.actual,
            &context.bootstrap.owner,
            conduit_body::HOST_PREPARED_WORKSET_CONTRACT,
            context.now_millis,
        )
        .map_err(ValidationRefusal::Retention)?;
    prepare_workset_storage(
        owner,
        installed,
        context,
        command,
        maxima,
        maximum_storage_bytes,
    )
}

pub fn prepare_workset_storage<'a, 'o>(
    owner: &'o mut crate::resident::generated::Initialized,
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'_, 'a>,
    maxima: c::AssignedPlanMaxima,
    maximum_storage_bytes: usize,
) -> Result<AttemptStorage<'a, 'o>, ValidationRefusal> {
    let checked = check_workset(owner, installed, context, command, maxima, None)?;
    storage::prepare(
        owner,
        checked.documents,
        checked.stamp,
        maximum_storage_bytes,
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
    check_workset(owner, installed, context, command, maxima, None).map(|_| ())
}

fn check_workset<'a>(
    owner: &crate::resident::generated::Initialized,
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'_, 'a>,
    maxima: c::AssignedPlanMaxima,
    execution: Option<ExecutionPolicy<'_>>,
) -> Result<CheckedWorkset<'a>, ValidationRefusal> {
    let workset = receive_delegated_body_workset::<{ crate::resident::generated::ENTRY_COUNT }>(
        installed, context, command,
    )
    .map_err(|_| ValidationRefusal::Workset)?;
    if let Some(execution) = execution {
        workset
            .check_execution_delegation(
                execution.installed,
                installed,
                context,
                execution.active_worksets,
            )
            .map_err(ValidationRefusal::Execution)?;
    }
    if workset.declared_runtime_state_bytes() > maxima.runtime_state_bytes {
        return Err(ValidationRefusal::Bounds);
    }
    let mut used = [false; crate::resident::generated::ENTRY_COUNT];
    let mut checked = core::array::from_fn(|_| None);
    for (index, output) in checked
        .iter_mut()
        .enumerate()
        .take(workset.fragment_count())
    {
        let document = workset.document(index).ok_or(ValidationRefusal::Document)?;
        let prepared = validate_document(owner, context, document, maxima, |profile| {
            workset
                .receive_fragment_with_approved_fore::<16>(index, installed, context, profile)
                .map_err(|_| ValidationRefusal::Assignment)
        })?;
        for (prior, new) in used.iter_mut().zip(prepared.used) {
            if *prior && new {
                return Err(ValidationRefusal::Selection);
            }
            *prior |= new;
        }
        *output = Some(prepared);
    }
    let stamp = workset
        .into_retained_stamp()
        .map_err(|_| ValidationRefusal::Workset)?;
    Ok(CheckedWorkset {
        documents: checked,
        stamp,
    })
}

struct CheckedWorkset<'a> {
    documents: [Option<CheckedDocument<'a>>; crate::resident::generated::ENTRY_COUNT],
    stamp: conduit_body::RetainedHostCommandStamp,
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
) -> Result<CheckedDocument<'a>, ValidationRefusal> {
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
    Ok(CheckedDocument {
        fragment,
        fore,
        selected,
        maxima,
        used,
    })
}

struct CheckedDocument<'a> {
    fragment: DelegatedBodyFragment<'a>,
    fore: ApprovedBodyForePorts<16>,
    selected: conduit_plan_lowering::resident_inventory::ResidentInventorySelection<
        { crate::resident::generated::ENTRY_COUNT },
    >,
    maxima: c::AssignedPlanMaxima,
    used: [bool; crate::resident::generated::ENTRY_COUNT],
}
impl CheckedDocument<'_> {
    fn profile(&self) -> AssignedBodyPreparationProfile<'_> {
        let pins = self.selected.computations();
        let metadata = self.fragment.metadata();
        AssignedBodyPreparationProfile {
            maxima: self.maxima,
            requirements: c::AssignedPlanRequirements {
                host: c::AssignedIdentity::from_text(metadata.host),
                boot: c::AssignedIdentity::from_text(metadata.boot),
                host_calls: &[],
                resources: &[0],
                remote_bindings: &[],
            },
            computations: c::AssignedComputationProfile {
                maximum_text_bytes: pins.iter().map(|p| p.text_bytes).max().unwrap_or(0),
                maximum_total_text_bytes: pins.iter().map(|p| p.text_bytes).sum(),
                requirements: pins,
            },
            fore: Some(self.fore.as_slice()),
        }
    }
}
