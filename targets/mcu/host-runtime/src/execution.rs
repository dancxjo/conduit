//! Shared whole-workset preparation and Core admission for constrained Hosts.
//! Exact Source factories supply actual prepared Back/native-owner loans. This
//! module uses the ordinary scheduler and original resource ledger throughout.
extern crate alloc;
use alloc::boxed::Box;
#[path = "execution_line_fore.rs"]
mod line_fore;
#[path = "execution_terminal_gate.rs"]
mod terminal_gate;
use conduit_body as b;
use conduit_core as c;
use conduit_kernel::{
    assigned::{BoundedAssignedKernelTables, ResidentLayout},
    scheduler::*,
    *,
};
use line_fore::ForeProfile;

/// A trusted native Source factory must validate every selected node against its
/// independently installed implementation/configuration and keep all actual
/// initialized owners borrowed until bind. Refusal/drop must return the owners.
///
/// # Safety
/// `bind` must transfer exactly those checked owners once, allocate nothing,
/// perform no effects and return drivers in the validated node order. A Some
/// terminal report must equal those exact drivers' reports, including every
/// selected and inactive slot, and remain immutable through bind. Reported
/// backing is reachable retained allocation, never an invented grant.
pub unsafe trait PreparedBacks<const N: usize, const P: usize> {
    type Driver: StepBack<P>;
    type Calls;
    /// Exact reports of the same owners later returned by bind. None refuses
    /// the terminal-aware entrance without withdrawing those owners.
    fn terminal_transductions(&self) -> Option<[[Option<AssignedTerminalTransduction>; P]; N]> {
        None
    }
    /// Fill one caller-owned matrix with the identical immutable actual-owner
    /// report returned by `terminal_transductions`, including inactive slots.
    /// None means uninspectable; partial writes on refusal are not a report.
    /// Implementations may avoid full-matrix return temporaries, but must not
    /// bind/withdraw owners, allocate, or change availability while inspecting.
    fn write_terminal_transductions(
        &self,
        target: &mut [[Option<AssignedTerminalTransduction>; P]; N],
    ) -> Option<()> {
        *target = self.terminal_transductions()?;
        Some(())
    }
    /// Terminal-bearing factories must never use the legacy empty-law entrance.
    fn requires_terminal_gate(&self) -> bool {
        false
    }
    /// Immutable physical requirements of exactly the drivers returned by bind.
    /// None is uninspectable, not an empty requirement or an execution grant.
    /// A Some report must remain exact through bind, including inactive slots.
    #[cfg(feature = "execution-workspace")]
    fn execution_workspace_requirements(
        &self,
    ) -> Option<[Option<conduit_kernel::scheduler::StepWorkspaceRequirements>; N]> {
        None
    }
    fn retained_backing_bytes(&self) -> Option<usize>;
    fn bind(self) -> ([Self::Driver; N], Self::Calls);
}
/// Physical backing outside the kernel's own inline storage.
/// # Safety
/// Count every external allocation retained by this exact Sign owner.
pub unsafe trait ExecutionSigns: SignSink {
    fn external_backing_bytes(&self) -> usize;
    fn events(&self) -> impl Iterator<Item = KernelEvent>;
}
// SAFETY: fixed arrays are wholly inside the kernel allocation.
unsafe impl<const S: usize> ExecutionSigns for FixedSignLog<S> {
    fn external_backing_bytes(&self) -> usize {
        0
    }
    fn events(&self) -> impl Iterator<Item = KernelEvent> {
        FixedSignLog::events(self)
    }
}
// SAFETY: the canonical store reports both actual backing capacities.
unsafe impl ExecutionSigns for HostedSignLog {
    fn external_backing_bytes(&self) -> usize {
        self.reserved_storage_bytes()
    }
    fn events(&self) -> impl Iterator<Item = KernelEvent> {
        HostedSignLog::events(self)
    }
}
/// `H` funds sparse binding addresses; `K` independently funds pending calls.
/// The default preserves existing inventories. Source may supply its checked
/// callable-node bound without changing operation identities or authority.
pub type ExecutionKernel<
    D,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    const S: usize,
    L = FixedSignLog<S>,
    const K: usize = H,
> = StoredExecutionKernel<D, L, N, C, P, R, T, H, K>;
/// The same scheduler independent of its prepared Sign representation.
pub type StoredExecutionKernel<
    D,
    L,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    const K: usize = H,
> = FixedScheduler<D, HostedValueStore, L, N, C, P, C, R, T, H, K>;
type Storage<
    B,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    const S: usize,
    L,
    const K: usize,
> = PreparedKernelStorage<
    <B as PreparedBacks<N, P>>::Driver,
    ExecutionKernel<<B as PreparedBacks<N, P>>::Driver, N, C, P, R, T, H, S, L, K>,
    N,
>;
#[derive(Clone, Copy)]
pub struct Policy<'a> {
    pub preparation: Option<&'a b::HostPreparationDelegation>,
    pub execution: Option<&'a b::HostExecutionDelegation>,
    pub active_worksets: u16,
    /// Successful retained kernel/Back backing and resource bridge, excluding
    /// independently admitted temporary preparation and allocator overhead.
    pub maximum_retained_bytes: usize,
}
#[derive(Debug)]
pub enum Refusal<E> {
    Retention(b::HostCommandRetentionRefusal),
    Workset(b::DelegatedWorksetRefusal),
    Execution(b::ExecutionDelegationRefusal),
    Resources(b::DelegatedResourceRefusal),
    Tables(conduit_kernel::assigned::AssignedKernelRefusal),
    Storage(KernelPreparationError),
    Back(E),
    Bounds,
    TerminalContracts,
    Capacity {
        required: usize,
        maximum: usize,
    },
    /// Atomic admission needs these additional bridge/observation buffers.
    /// This is the existing ceiling check, distinct from retained-only refusal.
    ResourcePreparationCapacity {
        retained: usize,
        bridge: usize,
        observations: usize,
        required: usize,
        maximum: usize,
    },
    /// A pre-allocation lower bound already exceeds the caller's retained
    /// execution budget; it does not claim to measure preparation peak.
    LowerBoundCapacity {
        required: usize,
        maximum: usize,
    },
    Scope,
    Mask(super::execution_mask::Refusal),
    Authority {
        available: usize,
        reason: super::execution_authority::Refusal,
    },
}
/// One prepared fragment inside a heterogeneous, atomically admitted workset.
/// This ticket has no independent admit/Start path; use PreparedLocalWorkset.
pub struct PreparedFragment<
    B: PreparedBacks<N, P>,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
    const S: usize,
    L: ExecutionSigns = FixedSignLog<S>,
    const K: usize = H,
> {
    storage: Storage<B, N, C, P, R, T, H, S, L, K>,
    backs: B,
    retained: usize,
    #[cfg(feature = "execution-workspace")]
    maximum_retained_bytes: usize,
}
impl<
        B: PreparedBacks<N, P>,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const S: usize,
        L: ExecutionSigns,
        const K: usize,
    > PreparedFragment<B, N, C, P, R, T, H, S, L, K>
{
    #[allow(clippy::too_many_arguments)]
    #[inline(never)] // Per-kernel table scratch must end before the next constructor.
    pub fn prepare<E>(
        approved: &b::DelegatedBodyFragment<'_>,
        profile: b::AssignedBodyPreparationProfile<'_>,
        fore: &[c::AssignedForeRequirement],
        layout: &ResidentLayout<'_>,
        values: HostedValueStore,
        signs: L,
        maximum_retained_bytes: usize,
        factory: impl FnOnce() -> Result<B, E>,
    ) -> Result<Self, Refusal<E>> {
        Self::prepare_inner(
            approved,
            profile,
            ForeProfile::Local(fore),
            layout,
            values,
            signs,
            maximum_retained_bytes,
            factory,
            None,
        )
    }

    /// Expected laws are independently linked from checked lowering, in the
    /// same pinned Source/node/port order as layout. They are not ASG terminal IDs.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with_terminal_transductions<E>(
        approved: &b::DelegatedBodyFragment<'_>,
        profile: b::AssignedBodyPreparationProfile<'_>,
        fore: &[c::AssignedForeRequirement],
        layout: &ResidentLayout<'_>,
        values: HostedValueStore,
        signs: L,
        maximum_retained_bytes: usize,
        factory: impl FnOnce() -> Result<B, E>,
        expected: [[Option<AssignedTerminalTransduction>; P]; N],
    ) -> Result<Self, Refusal<E>> {
        Self::prepare_inner(
            approved,
            profile,
            ForeProfile::Local(fore),
            layout,
            values,
            signs,
            maximum_retained_bytes,
            factory,
            Some(&expected),
        )
    }

    /// Borrow the independently linked immutable Source matrix. Unlike the
    /// existing by-value API, this needs no caller RAM matrix copy.
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    pub fn prepare_with_terminal_transductions_ref<E>(
        approved: &b::DelegatedBodyFragment<'_>,
        profile: b::AssignedBodyPreparationProfile<'_>,
        fore: &[c::AssignedForeRequirement],
        layout: &ResidentLayout<'_>,
        values: HostedValueStore,
        signs: L,
        maximum_retained_bytes: usize,
        factory: impl FnOnce() -> Result<B, E>,
        expected: &[[Option<AssignedTerminalTransduction>; P]; N],
    ) -> Result<Self, Refusal<E>> {
        Self::prepare_inner(
            approved,
            profile,
            ForeProfile::Local(fore),
            layout,
            values,
            signs,
            maximum_retained_bytes,
            factory,
            Some(expected),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_inner<E>(
        approved: &b::DelegatedBodyFragment<'_>,
        profile: b::AssignedBodyPreparationProfile<'_>,
        fore: ForeProfile<'_>,
        layout: &ResidentLayout<'_>,
        values: HostedValueStore,
        signs: L,
        maximum_retained_bytes: usize,
        factory: impl FnOnce() -> Result<B, E>,
        expected: Option<&[[Option<AssignedTerminalTransduction>; P]; N]>,
    ) -> Result<Self, Refusal<E>> {
        let backs = terminal_gate::prepare_checked_backs::<B, E, N, C, P, R, T, H>(
            approved.projection(),
            &profile,
            fore,
            layout,
            factory,
        )?;
        let empty = &[[None; P]; N];
        let expected = match expected {
            Some(expected) => {
                terminal_gate::check_actual::<B, N, P>(&backs, expected).map_err(|error| {
                    match error {
                        terminal_gate::ReportRefusal::Unavailable => Refusal::TerminalContracts,
                        terminal_gate::ReportRefusal::Mismatch => Refusal::Storage(
                            KernelPreparationError::Topology(SchedulerError::InvalidPlan),
                        ),
                    }
                })?;
                expected
            }
            None if backs.requires_terminal_gate() => return Err(Refusal::TerminalContracts),
            None => empty,
        };
        Self::prepare_storage(
            approved,
            profile,
            fore,
            layout,
            values,
            signs,
            maximum_retained_bytes,
            backs,
            expected,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    fn prepare_storage<E>(
        approved: &b::DelegatedBodyFragment<'_>,
        profile: b::AssignedBodyPreparationProfile<'_>,
        fore: ForeProfile<'_>,
        layout: &ResidentLayout<'_>,
        values: HostedValueStore,
        signs: L,
        maximum_retained_bytes: usize,
        backs: B,
        expected: &[[Option<AssignedTerminalTransduction>; P]; N],
    ) -> Result<Self, Refusal<E>> {
        // The independent actual-report scratch above is dead BEFORE table
        // construction. PreparedBacks guarantees these same reports are immutable
        // until bind. Kernel validation below still checks every inactive row.
        let backing = backs
            .retained_backing_bytes()
            .ok_or(Refusal::Bounds)?
            .checked_add(values.reserved_storage_bytes())
            .and_then(|n| n.checked_add(signs.external_backing_bytes()))
            .ok_or(Refusal::Bounds)?;
        let remaining = maximum_retained_bytes
            .checked_sub(backing)
            .ok_or(Refusal::Bounds)?;
        let storage =
            ExecutionKernel::<B::Driver, N, C, P, R, T, H, S, L, K>::prepare_boxed_with_table_writer(
                values,
                signs,
                remaining,
                expected,
                expected,
                Refusal::Storage,
                |nodes, cords, routes, calls| {
                    let identity =
                        fore.write::<N, C, P, R, T, H>(
                            approved.projection(),
                            &profile,
                            layout,
                            nodes,
                            cords,
                            routes,
                            calls,
                        )
                        .map_err(Refusal::Tables)?;
                    Ok((
                        usize::from(identity.counts[0]),
                        usize::from(identity.counts[3]),
                    ))
                },
            )?;
        let retained = backing
            .checked_add(storage.storage_bytes())
            .ok_or(Refusal::Bounds)?;
        Ok(Self {
            storage,
            backs,
            retained,
            #[cfg(feature = "execution-workspace")]
            maximum_retained_bytes,
        })
    }
}
// SAFETY: exact prepared tables retain the complete Back loans; binding only
// moves those owners into existing prepared storage, with no effects/Start.
unsafe impl<
        B: PreparedBacks<N, P>,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const S: usize,
        L: ExecutionSigns,
        const K: usize,
    > super::execution_workset::PreparedLocalKernels
    for PreparedFragment<B, N, C, P, R, T, H, S, L, K>
{
    type Bound = (
        Box<ExecutionKernel<B::Driver, N, C, P, R, T, H, S, L, K>>,
        B::Calls,
    );
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(self.retained)
    }
    #[inline(always)] // Fuse the affine owner transfer with its final destination.
    unsafe fn bind(self) -> Self::Bound {
        let (drivers, calls) = self.backs.bind();
        (self.storage.bind(drivers), calls)
    }
}

#[cfg(feature = "execution-workspace")]
#[path = "execution_workspace.rs"]
mod execution_workspace;
#[cfg(feature = "execution-workspace")]
pub use execution_workspace::WorkspaceFundingRefusal;
