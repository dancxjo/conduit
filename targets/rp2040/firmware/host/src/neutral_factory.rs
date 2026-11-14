//! Complete checked one-entry factory. Preparation never binds providers,
//! supplies Fore values, admits resources, or starts a partial workset.
use crate::{neutral_workset::*, resident::generated as resident};
use alloc::boxed::Box;
use conduit_body as b;
use conduit_i2c_transaction::{contract::I2cContract, installation::ReadyI2cBase, I2cProvider};
use conduit_kernel::{assigned::ResidentLayout, HostedSignLog, HostedValueStore, NodeId};
use conduit_mcu_host_runtime::{
    execution::{Policy, PreparedFragment, Refusal},
    execution_authority::{Issuer, NativeFacts},
    execution_factory::{
        PreparedI2cClockCalls, PreparedNativeDrivers, PreparedResidentSubset, PreparedResidents,
        SoftwareInventory,
    },
    execution_workset::{PreparedLocalKernels, PreparedLocalWorkset},
};
use conduit_monotonic_wait::clock_at::{
    contract::MonotonicClockContract, owner::MonotonicDeadlineProvider, retained::ReadyClockAtBase,
};
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_expression::SelectedAssignedExpressionPlan,
    selected_assigned_operation::{
        AssignedOperationInput, SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
    },
};

pub type Values = conduit_plan_lowering::shared_schema_value_store::SharedSchemaValueStore<
    'static,
    HostedValueStore,
>;

pub type Kernel<'a, 'f> = conduit_mcu_host_runtime::execution::ExecutionKernel<
    conduit_mcu_host_runtime::execution_factory::Driver<'f, resident::BorrowedBack<'a>>,
    N,
    C,
    P,
    R,
    T,
    H,
    S,
    HostedSignLog,
    K,
    Values,
    Q,
>;
pub type Bound<'a, 'f, I, D> = (
    Box<Kernel<'a, 'f>>,
    conduit_mcu_host_runtime::execution_factory::I2cClockCalls<I, D, 1, 1>,
);

/// Whole factory accepts original owners and opaque Root context. Generated
/// definitions/identities must be initialized together by the installed entry.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn prepare<'a, 'f: 'a, 'w, I: I2cProvider + 'a, D: MonotonicDeadlineProvider + 'a>(
    workspace: &'w b::HostCommandInput<{ crate::config::MAX_ASSIGNMENT_BYTES }>,
    context: b::NativePreparationContext<'_>,
    policy: Policy<'_>,
    residents: &'a mut resident::Initialized,
    software: &'a mut SoftwareInventory<'f, N>,
    bus: &'a mut Option<ReadyI2cBase<I>>,
    clock: &'a mut Option<ReadyClockAtBase<D>>,
    bus_issuer: &mut Issuer,
    clock_issuer: &mut Issuer,
    operations: &super::neutral_authority::Operations,
) -> Result<
    PreparedLocalWorkset<'w, impl PreparedLocalKernels<Bound = Bound<'a, 'f, I, D>> + 'a, 1>,
    Refusal<&'static str>,
> {
    let fore = layout::fore();
    let resources = b::InstalledPlainResourceTable::new(
        layout::INSTALLED_PLAIN_RESOURCE_BINDINGS.ok_or(Refusal::Back("plain-resource-profile"))?,
    )
    .map_err(|_| Refusal::Back("installed-resource-table"))?;
    PreparedLocalWorkset::prepare_selected_profiles::<
        { crate::config::MAX_ASSIGNMENT_BYTES },
        FORE_PORTS,
        _,
    >(
        workspace,
        context,
        policy,
        |index, _| {
            if index != 0 {
                return Err("neutral-profile-index");
            }
            Ok(conduit_mcu_host_runtime::execution_workset::AssignedPreparationProfile::InstalledPlainResources(
                b::AssignedBodyInstalledPlainResourcesProfile {
                    preparation: profile(context.actual, &fore),
                    resources,
                },
            ))
        },
        move |[approved], [approved_fore]| {
            let approved_fore = approved_fore.as_slice();
            let metadata = approved.metadata();
            // The exact independently compiled Source closure is authoritative.
            if metadata.source != layout::SOURCE_DOCUMENT_ID
                || metadata.checked_plot != layout::CHECKED_PLOT_ID
                || metadata.expanded_plot != layout::EXPANDED_PLOT_ID
            {
                return Err("neutral-source-pin");
            }
            let profile = profile(context.actual, approved_fore);
            let mut selected = [0u16; resident::ENTRY_COUNT];
            let mut selected_nodes = [false; N];
            for (entry, selection) in selected.iter_mut().enumerate() {
                let identity = residents.available_identity(entry).ok_or("resident-busy")?;
                // Program text can intentionally repeat. The independently
                // installed artifact pins this particular initialized owner;
                // select its exact assigned node before looking up its program.
                let mut assigned_node = None;
                for record in conduit_core::AssignedPlanRecords::new(approved.projection())
                    .map_err(|_| "resident-assigned-records")?
                {
                    let record = record.map_err(|_| "resident-assigned-records")?;
                    if record.tag != conduit_core::ASSIGNED_NODE {
                        continue;
                    }
                    let bytes = record.payload;
                    if bytes.len() != 52 {
                        return Err("resident-assigned-node");
                    }
                    if bytes[4..20] != identity.kind.0
                        || bytes[20..36] != identity.implementation.0
                        || bytes[36..52] != identity.artifact.0
                    {
                        continue;
                    }
                    let node = u16::from_le_bytes([bytes[0], bytes[1]]);
                    if usize::from(node) >= N || assigned_node.replace(node).is_some() {
                        return Err("resident-source-duplicate");
                    }
                }
                let node = assigned_node.ok_or("resident-source-pin")?;
                if core::mem::replace(&mut selected_nodes[usize::from(node)], true) {
                    return Err("resident-source-duplicate");
                }
                let mut matching = layout::COMPUTATIONS.iter().filter(|pin| {
                    pin.node == node
                        && pin.key == identity.computation.key
                        && pin.text_bytes == identity.computation.text_bytes
                        && pin.text_digest == identity.computation.text_digest
                });
                let pin = matching.next().ok_or("resident-source-pin")?;
                if matching.next().is_some() {
                    return Err("resident-source-duplicate");
                }
                *selection = pin.node;
            }
            // The complete identity/uniqueness pass above retains compact node
            // ordinals only. Shared original checked context creates one
            // transient assignment at a time; no full per-resident Plan array
            // survives preflight or duplicates the original projection/profile.
            let resident_loans = PreparedResidentSubset(
                residents
                    .prepare_selected_profiles_by(|entry| {
                        Ok(Some(resident::ResidentAssignment::Local(
                            SelectedAssignedExpressionPlan {
                                input: AssignedOperationInput::Contiguous(approved.projection()),
                                maxima: profile.maxima,
                                requirements: profile.requirements,
                                computations: profile.computations,
                                fore: Some(approved_fore),
                                fragment_id: metadata.fragment,
                                scope: AssignedPlayScope::Delegated(approved),
                                node: NodeId(selected[entry]),
                                work_resource: 0, // Original Root's checked work pool.
                            },
                        )))
                    })
                    .map_err(|_| "resident-assignment")?
                    .split(),
            );
            let resident_nodes = resident_loans.nodes();
            let node = |kind| -> Result<u16, &'static str> {
                let mut matches = source::NATIVE_KINDS
                    .iter()
                    .filter(|(_, actual)| *actual == kind);
                let selected = matches.next().ok_or("native-source-node")?.0;
                if matches.next().is_some() {
                    return Err("native-source-duplicate");
                }
                Ok(selected)
            };
            let operation = |node| SelectedAssignedForeOperationPlan {
                operation: SelectedAssignedOperationPlan {
                    input: AssignedOperationInput::Contiguous(approved.projection()),
                    maxima: profile.maxima,
                    requirements: profile.requirements,
                    computations: profile.computations,
                    fragment_id: metadata.fragment,
                    scope: AssignedPlayScope::Delegated(approved),
                    node: NodeId(node),
                },
                fore: approved_fore,
            };
            let (table, handle, claim) = bus_issuer
                .issue(
                    context,
                    policy,
                    approved,
                    &NativeFacts::i2c(bus.as_ref().ok_or("i2c-busy")?),
                    &operations.bus,
                    policy.maximum_retained_bytes,
                )
                .map_err(|_| "i2c-authority")?;
            let contract = I2cContract::prepare().map_err(|_| "i2c-contract")?;
            let bus = ReadyI2cBase::prepare_assigned_fore_retained(
                bus,
                table,
                handle,
                claim,
                &contract,
                operation(node("machine/i2c/transact")?),
            )
            .map_err(|_| "i2c-assignment")?;
            let (table, handle, claim) = clock_issuer
                .issue(
                    context,
                    policy,
                    approved,
                    &NativeFacts::clock_at(clock.as_ref().ok_or("clock-busy")?),
                    &operations.clock,
                    policy.maximum_retained_bytes,
                )
                .map_err(|_| "clock-authority")?;
            let contract = MonotonicClockContract::prepare().map_err(|_| "clock-contract")?;
            let clock = ReadyClockAtBase::prepare_assigned_fore_retained(
                clock,
                table,
                handle,
                claim,
                &contract,
                operation(node("machine/clock/at")?),
            )
            .map_err(|_| "clock-assignment")?;
            let mut software_nodes = [false; N];
            for identity in software.identities() {
                *software_nodes
                    .get_mut(usize::from(identity.node))
                    .ok_or("software-node")? = true;
            }
            let software = software.prepare_assigned(approved.projection(), software_nodes)?;
            let backs = PreparedNativeDrivers::new(
                resident_loans,
                resident_nodes,
                software,
                PreparedI2cClockCalls {
                    buses: [bus],
                    clocks: [clock],
                },
            )?;
            // Construct values only after inspecting the exact affine owners
            // selected by this authenticated assignment. The original logical
            // slots and canonical/native capacities remain unchanged.
            let (values, _, _stock) =
                conduit_mcu_host_runtime::native_values::prepare_shared_schema::<0, N, P>(
                    layout::CORD_VALUE_PROFILES,
                    layout::CORD_VALUE_SINKS,
                    layout::STEP_VALUE_PROFILES,
                    layout::STEP_VALUE_OUTPUTS,
                    layout::HOST_CALL_VALUE_SLOT_CAPACITIES,
                    &[],
                    policy.maximum_retained_bytes,
                    &backs,
                )?;
            // Kernel events and the Plan's named mandatory-Sign receipts are
            // separate representations. Fund the actual numeric event log as
            // in the shared Source execution entrance; do not reinterpret or
            // change the Plan's original SIGN_BYTES receipt budget.
            let event_items = u16::try_from(S).map_err(|_| "sign-bound")?;
            let event_bytes = S
                .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>())
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("sign-bound")?;
            let signs = HostedSignLog::new_with_remote_storage(
                event_items,
                event_bytes,
                event_items,
                conduit_kernel::remote_sign_storage_bytes(event_items).ok_or("sign-bound")?,
            )
            .map_err(|_| "sign-storage")?;
            let mut prepared = PreparedFragment::<
                _,
                N,
                C,
                P,
                R,
                T,
                H,
                S,
                HostedSignLog,
                K,
                Values,
                Q,
            >::prepare_with_terminal_transductions_ref(
                approved,
                profile,
                approved_fore,
                &ResidentLayout {
                    records: layout::RECORDS,
                    counts: layout::COUNTS,
                },
                values,
                signs,
                policy.maximum_retained_bytes,
                || Ok::<_, &'static str>(backs),
                &TERMINALS,
            )
            .map_err(|_| "neutral-kernel")?;
            prepared
                .prepare_execution_workspace()
                .map_err(|_| "neutral-workspace")?;
            Ok(prepared)
        },
    )
}
