// Shared composition extracted from the neutral board factory: retain original
// resident/software/native owners, construct storage from those exact owners,
// and enter the original kernel preparation path with immutable terminal laws.
// This returns an unbound member of a whole-workset preparation, never a Play.
// Callers selecting execution-workspace must additionally use the original
// prepare_execution_workspace gate before whole-workset admission and Start.
use conduit_kernel::{assigned::ResidentLayout, scheduler::StepBack};
use conduit_mcu_host_runtime::{
    execution::{ExecutionSigns, ExecutionValues, PreparedBacks, PreparedFragment, Refusal},
    execution_factory::{PreparedNativeDrivers, PreparedResidents, PreparedSoftware},
    execution_group::EntryPin,
};

pub type I2cClockDrivers<'o, 'f, R, I, D, const E: usize, const N: usize> =
    PreparedNativeDrivers<'o, 'f, R, PreparedI2cClockCalls<'o, I, D, 1, 1>, E, N>;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn prepare_fragment<
    'o,
    'f,
    'work,
    R: PreparedResidents<E>,
    I: I2cProvider,
    D: MonotonicDeadlineProvider,
    V: ExecutionValues,
    L: ExecutionSigns,
    const E: usize,
    const N: usize,
    const CORDS: usize,
    const P: usize,
    const ROUTES: usize,
    const TARGETS: usize,
    const CALLS: usize,
    const SIGNS: usize,
    const PENDING: usize,
    const SLOTS: usize,
    const PHASE: usize,
    const UNRESOLVED: usize,
    const TERMINALS: usize,
>(
    context: b::NativePreparationContext<'_>,
    policy: Policy<'_>,
    approved: &'work b::DelegatedBodyFragment<'work>,
    profile: &b::AssignedBodyPreparationProfile<'work>,
    fore: &'work [c::AssignedForeRequirement],
    bus: &'o mut Option<ReadyI2cBase<I>>,
    clock: &'o mut Option<ReadyClockAtBase<D>>,
    bus_issuer: &mut Issuer,
    clock_issuer: &mut Issuer,
    bus_operation: &Operation<'_>,
    clock_operation: &Operation<'_>,
    bus_contract: &I2cContract,
    clock_contract: &MonotonicClockContract,
    residents: R,
    software: PreparedSoftware<'o, 'f, N>,
    kernel_layout: &ResidentLayout<'_>,
    expected: &'static [[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]; N],
    storage: impl FnOnce(&I2cClockDrivers<'o, 'f, R, I, D, E, N>, usize) -> Result<(V, L), &'static str>,
) -> Result<
    PreparedFragment<
        I2cClockDrivers<'o, 'f, R, I, D, E, N>,
        N,
        CORDS,
        P,
        ROUTES,
        TARGETS,
        CALLS,
        SIGNS,
        L,
        PENDING,
        V,
        SLOTS,
        0,
        PHASE,
        UNRESOLVED,
        TERMINALS,
    >,
    Refusal<&'static str>,
>
where
    R::Back: StepBack<P>,
{
    EntryPin {
        source: layout::SOURCE_DOCUMENT_ID,
        checked: layout::CHECKED_PLOT_ID,
        expanded: layout::EXPANDED_PLOT_ID,
    }
    .check_configuration(approved, layout::CONFIGURATIONS)?;
    if N != source::NODE_COUNT || CORDS != source::CORD_COUNT {
        return Err(Refusal::Scope);
    }
    // Each successive constructor receives only the original remaining budget.
    // Complete kernel preparation independently charges all external backing
    // plus the exact requested kernel geometry, including inline storage.
    let native_budget = policy
        .maximum_retained_bytes
        .checked_sub(residents.retained_backing_bytes().ok_or(Refusal::Bounds)?)
        .and_then(|n| n.checked_sub(software.retained_backing_bytes()))
        .ok_or(Refusal::Bounds)?;
    let native = prepare(
        context,
        policy,
        approved,
        profile,
        fore,
        bus,
        clock,
        bus_issuer,
        clock_issuer,
        bus_operation,
        clock_operation,
        bus_contract,
        clock_contract,
        native_budget,
    )
    .map_err(Refusal::Back)?;
    let resident_nodes = residents.nodes();
    let backs = PreparedNativeDrivers::new(residents, resident_nodes, software, native)
        .map_err(Refusal::Back)?;
    let remaining = policy.maximum_retained_bytes
        .checked_sub(<I2cClockDrivers<'o, 'f, R, I, D, E, N> as PreparedBacks<N, P>>::retained_backing_bytes(&backs)
            .ok_or(Refusal::Bounds)?)
        .ok_or(Refusal::Bounds)?;
    let (values, signs) = storage(&backs, remaining).map_err(Refusal::Back)?;
    PreparedFragment::prepare_with_linked_terminal_transductions(
        approved,
        b::AssignedBodyPreparationProfile {
            maxima: profile.maxima,
            requirements: profile.requirements,
            computations: profile.computations,
            fore: profile.fore,
        },
        fore,
        kernel_layout,
        values,
        signs,
        policy.maximum_retained_bytes,
        || Ok(backs),
        expected,
    )
}
