// Checked Source supplies layout and remapped native nodes in the enclosing
// module. This constructor prepares original affine owners; it neither binds
// providers nor admits or starts a Play. Sensor protocols remain authored Plots.
use conduit_body as b;
use conduit_core as c;
use conduit_i2c_transaction::{
    contract::{I2cContract, I2C_CALL, I2C_KIND},
    installation::ReadyI2cBase,
    I2cProvider,
};
use conduit_mcu_host_runtime::{
    execution::Policy,
    execution_authority::{Issuer, NativeFacts, Operation},
    execution_factory::PreparedI2cClockCalls,
};
use conduit_monotonic_wait::clock_at::{
    contract::{MonotonicClockContract, CLOCK_CALL, CLOCK_KIND},
    owner::MonotonicDeadlineProvider,
    retained::ReadyClockAtBase,
};
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_operation::{
        AssignedOperationInput, SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
    },
};

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn prepare<'a, 'f, I: I2cProvider, C: MonotonicDeadlineProvider>(
    context: b::NativePreparationContext<'_>,
    policy: Policy<'_>,
    approved: &'f b::DelegatedBodyFragment<'f>,
    profile: &b::AssignedBodyPreparationProfile<'f>,
    fore: &'f [c::AssignedForeRequirement],
    bus: &'a mut Option<ReadyI2cBase<I>>,
    clock: &'a mut Option<ReadyClockAtBase<C>>,
    bus_issuer: &mut Issuer,
    clock_issuer: &mut Issuer,
    bus_operation: &Operation<'_>,
    clock_operation: &Operation<'_>,
    bus_contract: &I2cContract,
    clock_contract: &MonotonicClockContract,
    maximum_backing_bytes: usize,
) -> Result<PreparedI2cClockCalls<'a, I, C, 1, 1>, &'static str> {
    let (bus_node, clock_node) = selected_nodes(source::NATIVE_KINDS)?;
    let metadata = approved.metadata();
    if metadata.source != layout::SOURCE_DOCUMENT_ID
        || metadata.checked_plot != layout::CHECKED_PLOT_ID
        || metadata.expanded_plot != layout::EXPANDED_PLOT_ID
        || bus_operation.kind != I2C_KIND
        || bus_operation.call != I2C_CALL
        || clock_operation.kind != CLOCK_KIND
        || clock_operation.call != CLOCK_CALL
    {
        return Err("native-source-selection");
    }
    // Refuse absent equipment before issuing either grant. Attempt counters may
    // advance on later refusal; exclusive Ready slots remain intact throughout.
    if bus.is_none() || clock.is_none() {
        return Err("native-owner-busy");
    }
    let operation = |node| SelectedAssignedForeOperationPlan {
        operation: SelectedAssignedOperationPlan {
            input: AssignedOperationInput::Contiguous(approved.projection()),
            maxima: profile.maxima,
            requirements: profile.requirements,
            computations: profile.computations,
            fragment_id: metadata.fragment,
            scope: AssignedPlayScope::Delegated(approved),
            node: conduit_kernel::NodeId(node),
        },
        fore,
    };
    let (table, handle, claim) = bus_issuer
        .issue(
            context,
            policy,
            approved,
            &NativeFacts::i2c(bus.as_ref().ok_or("native-owner-busy")?),
            bus_operation,
            maximum_backing_bytes,
        )
        .map_err(|_| "i2c-authority")?;
    let bus = ReadyI2cBase::prepare_assigned_fore_retained(
        bus,
        table,
        handle,
        claim,
        bus_contract,
        operation(bus_node),
    )
    .map_err(|_| "i2c-assignment")?;
    let remaining = maximum_backing_bytes
        .checked_sub(
            bus.retained_backing_bytes()
                .ok_or("native-backing-overflow")?,
        )
        .ok_or("native-backing-bounds")?;
    let (table, handle, claim) = clock_issuer
        .issue(
            context,
            policy,
            approved,
            &NativeFacts::clock_at(clock.as_ref().ok_or("native-owner-busy")?),
            clock_operation,
            remaining,
        )
        .map_err(|_| "clock-authority")?;
    let clock = ReadyClockAtBase::prepare_assigned_fore_retained(
        clock,
        table,
        handle,
        claim,
        clock_contract,
        operation(clock_node),
    )
    .map_err(|_| "clock-assignment")?;
    remaining
        .checked_sub(
            clock
                .retained_backing_bytes()
                .ok_or("native-backing-overflow")?,
        )
        .ok_or("native-backing-bounds")?;
    Ok(PreparedI2cClockCalls {
        buses: [bus],
        clocks: [clock],
    })
}

// Require the complete native inventory, not merely the first matching bus.
// A shared authored protocol must own its one controller and one deadline; any
// additional native equipment needs a separately prepared constructor shape.
fn selected_nodes(kinds: &[(u16, &str)]) -> Result<(u16, u16), &'static str> {
    match kinds {
        [(bus, I2C_KIND), (clock, CLOCK_KIND)] | [(clock, CLOCK_KIND), (bus, I2C_KIND)]
            if bus != clock =>
        {
            Ok((*bus, *clock))
        }
        _ => Err("native-source-inventory"),
    }
}
