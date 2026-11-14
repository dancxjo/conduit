//! Original Root/native preparation independent of timers, carrier or board pins.
use crate::{
    neutral_authority::{Issuers, Operations},
    neutral_equipment::Equipment,
    neutral_workset::{layout, source, N},
};
use conduit_body::NativePreparationContext;
use conduit_i2c_transaction::I2cProvider;
use conduit_mcu_host_runtime::{
    execution::Policy,
    execution_factory::{SoftwareDefinition, SoftwareInventory},
    root::Root,
};
use conduit_monotonic_wait::clock_at::owner::MonotonicDeadlineProvider;
use core::cell::RefCell;
pub struct Runtime {
    pub now: fn() -> u64,
    pub allocated_bytes: fn() -> usize,
    pub challenge: fn() -> [u8; 32],
}
/// CPU qualification hook uses the same original Root/owners and pins as boot.
/// It neither fabricates a command nor binds providers. Successful preflight is
/// not admission; all affine loans are returned before this function returns.
#[allow(clippy::too_many_arguments)]
#[inline(never)] // Keep cold construction off the normal control polling frame.
pub fn preflight<I: I2cProvider, D: MonotonicDeadlineProvider>(
    root: &mut Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    equipment: &mut Equipment<I, D>,
    residents: &mut crate::resident::generated::Initialized,
    issuers: &mut Issuers,
    operations: &Operations,
    runtime: &Runtime,
    maximum_bytes: usize,
) -> Result<(), &'static str> {
    let parts = root
        .execution_parts()
        .map_err(|_| "current-member-required")?;
    let actual = parts.resources.host();
    let context = NativePreparationContext {
        bootstrap: parts.bootstrap,
        member: parts.member,
        actual,
        image_id: parts.configuration.image_id,
        configuration_id: parts.configuration.configuration_id,
        now_millis: (runtime.now)(),
    };
    let policy = Policy {
        preparation: parts.configuration.preparation,
        execution: parts.configuration.execution,
        active_worksets: 0,
        maximum_retained_bytes: maximum_bytes,
    };

    // This installed entrance has no Face, recording or periodic wait owners.
    // Refuse such material explicitly, never create an unbound surrogate owner.
    for index in 0..N {
        if let Some(definition) = source::definition_at(index, &[])? {
            if !matches!(
                definition,
                SoftwareDefinition::LinkedSourceStatic { .. }
                    | SoftwareDefinition::SeededUntilStatic { .. }
                    | SoftwareDefinition::MergeStatic { .. }
                    | SoftwareDefinition::ZipStatic { .. }
                    | SoftwareDefinition::SelectorStatic { .. }
            ) {
                return Err("unsupported-software-owner");
            }
        }
    }
    let publisher = RefCell::new(None);
    let mut software = SoftwareInventory::<N>::empty();
    // SAFETY: definitions, identities and configuration are emitted together
    // from the pinned checked entry. Static schema decoding occurs inside the
    // original measured initializer. Caller funds the whole preparation arena;
    // this synchronous section has exclusive allocator use just as board boot.
    unsafe {
        software.initialize_by(
            |index| source::definition_at(index, &[]),
            |index| source::identity_at(index, &[]),
            &publisher,
            conduit_environment_runtime::publication::PublicationSequence::prepare(0),
            runtime.now,
            runtime.allocated_bytes,
            source::CONFIGURATION,
            None,
        )?;
    }
    let (values, _) = conduit_mcu_host_runtime::native_values::prepare_schema(
        layout::CORD_VALUE_PROFILES,
        layout::STEP_VALUE_PROFILES,
        layout::HOST_CALL_VALUE_SLOT_CAPACITIES,
        &[],
        maximum_bytes,
    )?;
    let prepared = crate::neutral_factory::prepare(
        parts.workspace,
        context,
        policy,
        residents,
        &mut software,
        &mut equipment.bus,
        &mut equipment.clock,
        &mut issuers.bus,
        &mut issuers.clock,
        operations,
        values,
    )
    .map_err(|_| "neutral-preparation-refused")?;
    // No Fore service -> no admission and no provider bind. Parent Fore bridge
    // must attach actual admitted ingress/egress before this drop becomes Start.
    drop(prepared);
    Ok(())
}
