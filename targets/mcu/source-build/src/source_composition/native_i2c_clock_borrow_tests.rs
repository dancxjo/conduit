//! Compiler-only Root callback contract: original opaque approval, short Root
//! loan, independently retained static software definitions. No token is made,
//! no provider is initialized, and this helper is never executed.
use super::*;

#[allow(dead_code, clippy::too_many_arguments, clippy::type_complexity)]
fn short_root_approval_with_static_software<'owner, 'root, R, I, D, const E: usize>(
    context: b::NativePreparationContext<'_>,
    policy: Policy<'_>,
    approved: &'root b::DelegatedBodyFragment<'root>,
    profile: &b::AssignedBodyPreparationProfile<'root>,
    fore: &'root [c::AssignedForeRequirement],
    software: PreparedSoftware<'owner, 'static, 20>,
    residents: R,
    bus: &'owner mut Option<ReadyI2cBase<I>>,
    clock: &'owner mut Option<ReadyClockAtBase<D>>,
    issuers: (&mut Issuer, &mut Issuer),
    operations: (&Operation<'_>, &Operation<'_>),
    contracts: (&I2cContract, &MonotonicClockContract),
    kernel_layout: &ResidentLayout<'_>,
    values: conduit_kernel::HostedValueStore,
    signs: conduit_kernel::FixedSignLog<16>,
) -> Result<
    impl conduit_mcu_host_runtime::execution_workset::PreparedLocalKernels + 'owner,
    Refusal<&'static str>,
>
where
    R: PreparedResidents<E> + 'owner,
    R::Back: StepBack<2> + 'owner,
    I: I2cProvider + 'owner,
    D: MonotonicDeadlineProvider + 'owner,
{
    prepare_fragment::<R, I, D, _, _, E, 20, 1, 2, 40, 1, 20, 16, 20, 1, 0, 0, 1>(
        context,
        policy,
        approved,
        profile,
        fore,
        bus,
        clock,
        issuers.0,
        issuers.1,
        operations.0,
        operations.1,
        contracts.0,
        contracts.1,
        residents,
        software,
        kernel_layout,
        &[[None; 2]; 20],
        move |_, _| Ok((values, signs)),
    )
}
