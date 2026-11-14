//! Original grants derived from this IMAGE's checked Source and actual owners.
use crate::equipment::Equipment;
use conduit_mcu_host_runtime::{
    execution::Policy,
    native_authority::{Issuers, Operations},
    root::{Refusal, Root},
};
pub type NativeIssuers = Issuers<{ crate::equipment::SOURCE_WAIT_SLOTS }>;
pub type NativeOperations = Operations<'static, { crate::equipment::SOURCE_WAIT_SLOTS }>;

pub fn operations() -> NativeOperations {
    let clocks = core::array::from_fn(|slot| crate::workset::sampling::CLOCKS.get(slot).copied());
    Operations::from_source(
        crate::workset::sampling::NATIVE_KINDS,
        crate::workset::mask::NATIVE_KINDS,
        crate::workset::output::NATIVE_KINDS,
        clocks,
        crate::config::RENDERER_MAXIMUM_FRAMES,
        crate::config::RENDERER_FACE_BYTES,
        crate::config::RENDERER_SHOW_BYTES,
    )
    .expect("exact installed Source native operations")
}
pub fn authorize<const N: usize>(
    root: &mut Root<'_, N>,
    equipment: &Equipment,
    issuers: &mut NativeIssuers,
    operations: &NativeOperations,
) -> Result<(), Refusal> {
    issuers
        .authorize(
            root.return_context(conduit_esp32_electrical::platform::monotonic_millis())?,
            Policy {
                preparation: root.configuration().preparation,
                execution: root.configuration().execution,
                active_worksets: 0,
                maximum_retained_bytes: crate::config::MAX_RUNTIME_STATE_BYTES,
            },
            equipment.pulse.as_ref().ok_or(Refusal::Scope)?,
            equipment.display.as_ref().ok_or(Refusal::Scope)?,
            &equipment.renderer,
            &equipment.waits,
            operations,
            || crate::config::HEAP_BYTES.saturating_sub(crate::runtime::allocator_used()),
        )
        .map_err(|_| Refusal::Scope)
}
