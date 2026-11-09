//! Shared whole-workset preparation and Core admission for constrained Hosts.
//! Exact Source factories supply actual prepared Back/native-owner loans. This
//! module uses the ordinary scheduler and original resource ledger throughout.
extern crate alloc;
use alloc::boxed::Box;
use conduit_body as b;
use conduit_core as c;
use conduit_kernel::{
    assigned::{BoundedAssignedKernelTables, ResidentLayout},
    scheduler::*,
    *,
};

/// A trusted native Source factory must validate every selected node against its
/// independently installed implementation/configuration and keep all actual
/// initialized owners borrowed until bind. Refusal/drop must return the owners.
///
/// # Safety
/// `bind` must transfer exactly those checked owners once, allocate nothing,
/// perform no effects and return drivers in the validated node order. Reported
/// backing is reachable retained allocation, never an invented grant.
pub unsafe trait PreparedBacks<const N: usize, const P: usize> {
    type Driver: StepBack<P>;
    type Calls;
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
> = StoredExecutionKernel<D, L, N, C, P, R, T, H>;
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
> = FixedScheduler<D, HostedValueStore, L, N, C, P, C, R, T, H, H>;
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
> = PreparedKernelStorage<
    <B as PreparedBacks<N, P>>::Driver,
    ExecutionKernel<<B as PreparedBacks<N, P>>::Driver, N, C, P, R, T, H, S, L>,
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
    Capacity {
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
> {
    storage: Storage<B, N, C, P, R, T, H, S, L>,
    backs: B,
    retained: usize,
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
    > PreparedFragment<B, N, C, P, R, T, H, S, L>
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
        let tables = BoundedAssignedKernelTables::<N, C, P, R, T, H>::prepare_with_fore(
            approved.projection(),
            profile.maxima,
            profile.requirements,
            profile.computations,
            layout,
            fore,
        )
        .map_err(Refusal::Tables)?;
        let nodes = tables.active_nodes();
        let cords = tables.active_cords();
        let backs = factory().map_err(Refusal::Back)?;
        let backing = backs
            .retained_backing_bytes()
            .ok_or(Refusal::Bounds)?
            .checked_add(values.reserved_storage_bytes())
            .and_then(|n| n.checked_add(signs.external_backing_bytes()))
            .ok_or(Refusal::Bounds)?;
        let remaining = maximum_retained_bytes
            .checked_sub(backing)
            .ok_or(Refusal::Bounds)?;
        let storage = ExecutionKernel::<B::Driver, N, C, P, R, T, H, S, L>::prepare_boxed(
            nodes,
            cords,
            tables.nodes,
            tables.cords,
            tables.routes,
            tables.calls,
            values,
            signs,
            remaining,
        )
        .map_err(Refusal::Storage)?;
        let retained = backing
            .checked_add(storage.storage_bytes())
            .ok_or(Refusal::Bounds)?;
        Ok(Self {
            storage,
            backs,
            retained,
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
    > super::execution_workset::PreparedLocalKernels
    for PreparedFragment<B, N, C, P, R, T, H, S, L>
{
    type Bound = (
        Box<ExecutionKernel<B::Driver, N, C, P, R, T, H, S, L>>,
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
