//! Complete signed-set approval and Core admission for the bounded mixed profile.
//! No provider transfer precedes atomic admission; no successful result is Start.
use crate::{
    mixed_preparation::{BoundMixedKernel, PreparedMixedKernel},
    native_preparation::{NativePossession, Refusal as NativeRefusal},
    resident::generated,
};
use conduit_body as b;
#[path = "mixed_execution.rs"]
mod execution;
use conduit_core as c;
use conduit_i2c_transaction::{
    contract::{I2cContract, I2C_CALL},
    I2cProvider,
};
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_operation::{
        AssignedOperationInput, SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
    },
};
pub use execution::{MixedStep, RetiredMixedWorkset, StartedMixedWorkset};
#[path = "mixed_resources.rs"]
mod resources;
pub use resources::OwnedObservations;

/// Installed Root facts and remaining preparation capacity, never carrier fields.
#[derive(Clone, Copy)]
pub struct Policy<'a> {
    pub preparation: Option<&'a b::HostPreparationDelegation>,
    pub execution: Option<&'a b::HostExecutionDelegation>,
    pub active_worksets: u16,
    pub work_pool: &'a str,
    pub maxima: c::AssignedPlanMaxima,
    pub maximum_storage_bytes: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Retention(b::HostCommandRetentionRefusal),
    Workset(b::DelegatedWorksetRefusal),
    Execution(b::ExecutionDelegationRefusal),
    Bounds,
    Document,
    Native(NativeRefusal),
    Resources(b::DelegatedResourceRefusal),
}
pub struct PreparedMixedWorkset<'w, 'n, 'o, P: I2cProvider> {
    approved: b::DelegatedBodyFragment<'w>,
    kernel: PreparedMixedKernel<'n, 'o, P>,
    stamp: b::RetainedHostCommandStamp,
    maximum_storage_bytes: usize,
}
pub struct CommittedMixedWorkset<'w, 'o, P> {
    pub approved: b::DelegatedBodyFragment<'w>,
    pub bound: BoundMixedKernel<'o, P>,
    pub stamp: b::RetainedHostCommandStamp,
}
impl<P: I2cProvider> PreparedMixedWorkset<'_, '_, '_, P> {
    pub fn kernel_storage_bytes(&self) -> usize {
        self.kernel.storage_bytes()
    }
    /// Caller observations, installed inventory, inline Root storage and allocator
    /// overhead remain separately funded. This charges the existing bridge/Core
    /// requested backing conservatively before it allocates any request table.
    pub fn preparation_bytes(
        &self,
        owner: &c::ResourceAdmissionOwner,
        observations: &[c::ResourceObservation],
    ) -> Result<usize, Refusal> {
        let bridge =
            b::delegated_plain_resource_preparation_bytes(&[&self.approved], owner, observations)
                .map_err(Refusal::Resources)?;
        self.kernel
            .storage_bytes()
            .checked_add(bridge)
            .ok_or(Refusal::Bounds)
    }
}
impl<'w, 'n, 'o, P: I2cProvider> PreparedMixedWorkset<'w, 'n, 'o, P> {
    /// Core reserves all selected placements atomically into its original ledger.
    /// Refusal destroys only this unstarted preparation; both owners remain ready.
    /// Success retains attribution and approval, and binds without fallible work.
    pub fn admit_and_bind(
        self,
        owner: &mut c::ResourceAdmissionOwner,
        observations: &[c::ResourceObservation],
    ) -> Result<CommittedMixedWorkset<'w, 'o, P>, Refusal> {
        if self.preparation_bytes(owner, observations)? > self.maximum_storage_bytes {
            return Err(Refusal::Bounds);
        }
        b::admit_delegated_plain_resources(&[&self.approved], owner, observations)
            .map_err(Refusal::Resources)?;
        Ok(CommittedMixedWorkset {
            approved: self.approved,
            bound: self.kernel.bind(),
            stamp: self.stamp,
        })
    }
}

/// This installed representation supports exactly one complete local fragment,
/// containing one resident computation and one native operation. Extra fragments
/// refuse as a complete set, never silently disappear into partial execution.
pub fn prepare_retained<'w, 'n, 'o, P: I2cProvider, const N: usize>(
    owner: &'o mut generated::Initialized,
    possession: NativePossession<'n, P>,
    contract: &I2cContract,
    policy: Policy<'_>,
    context: b::NativePreparationContext<'_>,
    workspace: &'w b::HostCommandInput<N>,
) -> Result<PreparedMixedWorkset<'w, 'n, 'o, P>, Refusal> {
    let NativePossession {
        slot,
        table,
        handle,
        claim,
    } = possession;
    prepare_retained_with_issuer(owner, slot, contract, policy, context, workspace, |_, _| {
        Ok((table, handle, claim))
    })
}

/// Root's local issuer is called only after complete attributed workset and
/// execution-policy approval, initialized selection and exact resource pins.
/// The issuer must independently enforce installed native authority and funding;
/// the approved document itself never grants possession. Refusal retains both
/// providers and performs no Core admission, transfer, Start or bus operation.
pub fn prepare_retained_with_issuer<'w, 'n, 'o, P: I2cProvider, const N: usize>(
    owner: &'o mut generated::Initialized,
    slot: &'n mut Option<conduit_i2c_transaction::installation::ReadyI2cBase<P>>,
    contract: &I2cContract,
    policy: Policy<'_>,
    context: b::NativePreparationContext<'_>,
    workspace: &'w b::HostCommandInput<N>,
    issue: impl FnOnce(
        &b::DelegatedBodyFragment<'w>,
        &conduit_i2c_transaction::installation::ReadyI2cBase<P>,
    ) -> Result<
        (
            c::BaseCapabilityTable,
            c::BaseCapabilityHandle,
            c::BaseOperationClaim,
        ),
        NativeRefusal,
    >,
) -> Result<PreparedMixedWorkset<'w, 'n, 'o, P>, Refusal> {
    let command = workspace
        .reborrow(
            context.member,
            context.actual,
            &context.bootstrap.owner,
            b::HOST_PREPARED_WORKSET_CONTRACT,
            context.now_millis,
        )
        .map_err(Refusal::Retention)?;
    let workset = b::receive_delegated_body_workset::<1>(policy.preparation, context, command)
        .map_err(Refusal::Workset)?;
    workset
        .check_execution_delegation(
            policy.execution,
            policy.preparation,
            context,
            policy.active_worksets,
        )
        .map_err(Refusal::Execution)?;
    if workset.fragment_count() != 1
        || workset.declared_runtime_state_bytes() > policy.maxima.runtime_state_bytes
    {
        return Err(Refusal::Bounds);
    }
    let document = workset.document(0).ok_or(Refusal::Document)?;
    let (_, projection) =
        b::decode_body_fragment_document(document).map_err(|_| Refusal::Document)?;
    let native = slot
        .as_ref()
        .ok_or(Refusal::Native(NativeRefusal::Assignment))?;
    // Both bounded positions are tried against the same independently retained
    // native identity. No incoming kind/implementation label creates that hint.
    let mut selection = None;
    for node in 0..2 {
        let hint = native.assigned_node_identity(node);
        if let Ok(residents) = owner.identify_mixed_assignments(projection, &[hint]) {
            if selection.is_some() {
                return Err(Refusal::Bounds);
            }
            selection = Some((node, residents));
        }
    }
    let (node, residents) = selection.ok_or(Refusal::Native(NativeRefusal::Assignment))?;
    if residents.nodes().count() != 1 {
        return Err(Refusal::Bounds);
    }
    let computations = c::AssignedComputationProfile {
        maximum_text_bytes: residents
            .computations()
            .iter()
            .map(|p| p.text_bytes)
            .max()
            .ok_or(Refusal::Bounds)?,
        maximum_total_text_bytes: residents
            .computations()
            .iter()
            .try_fold(0u32, |n, p| n.checked_add(p.text_bytes))
            .ok_or(Refusal::Bounds)?,
        requirements: residents.computations(),
    };
    let calls = [c::AssignedIdentity::from_text(I2C_CALL)];
    // Each selected unary implementation has its own single resource binding.
    // The metadata/Core bridge independently checks the actual pool/class/units.
    let requirements = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text(context.actual.host_id.as_str()),
        boot: c::AssignedIdentity::from_text(context.actual.boot_id.as_str()),
        host_calls: &calls,
        resources: &[0, 0],
        remote_bindings: &[],
    };
    let (approved, fore) = workset
        .receive_fragment_with_approved_fore::<2>(
            0,
            policy.preparation,
            context,
            b::AssignedBodyPreparationProfile {
                maxima: policy.maxima,
                requirements,
                computations,
                fore: None,
            },
        )
        .map_err(Refusal::Workset)?;
    // Numeric resource slots alone do not identify the actual reserved pool.
    // Pin both metadata entries to independently held local owners before any
    // codec/kernel allocation. The native binder separately checks possession.
    let resident = residents.nodes().next().ok_or(Refusal::Bounds)?;
    let work = owner
        .available_identity(resident.entry)
        .ok_or(Refusal::Bounds)?
        .private_work_units;
    let mut seen = [0u8; 2];
    for entry in b::PlainFragmentResources::new(approved.metadata().resource_bindings) {
        let entry =
            entry.map_err(|_| Refusal::Resources(b::DelegatedResourceRefusal::Projection))?;
        let valid = if entry.node == node {
            seen[0] += 1;
            entry.resource == 0
                && entry.pool == native.native_identity().resource_pool_id.as_str()
                && entry.class == conduit_i2c_transaction::installation::I2C_ATTACHMENT_CLASS
                && entry.units == 1
        } else if entry.node == resident.node {
            seen[1] += 1;
            entry.resource == 0
                && entry.pool == policy.work_pool
                && entry.class == c::COOPERATIVE_STEP_WORK_CLASS_ID
                && entry.units == u32::from(work)
        } else {
            false
        };
        if !valid {
            return Err(Refusal::Resources(b::DelegatedResourceRefusal::Scope));
        }
    }
    if seen != [1, 1] {
        return Err(Refusal::Resources(b::DelegatedResourceRefusal::Scope));
    }
    let (table, handle, claim) = issue(&approved, native).map_err(Refusal::Native)?;
    let kernel = PreparedMixedKernel::prepare(
        owner,
        NativePossession {
            slot,
            table,
            handle,
            claim,
        },
        contract,
        SelectedAssignedForeOperationPlan {
            operation: SelectedAssignedOperationPlan {
                input: AssignedOperationInput::Contiguous(approved.projection()),
                maxima: policy.maxima,
                requirements,
                computations,
                fragment_id: approved.metadata().fragment,
                scope: AssignedPlayScope::Delegated(&approved),
                node: conduit_kernel::NodeId(node),
            },
            fore: fore.as_slice(),
        },
        0,
        policy.maximum_storage_bytes,
    )
    .map_err(Refusal::Native)?;
    let stamp = workset.into_retained_stamp().map_err(Refusal::Retention)?;
    Ok(PreparedMixedWorkset {
        approved,
        kernel,
        stamp,
        maximum_storage_bytes: policy.maximum_storage_bytes,
    })
}
