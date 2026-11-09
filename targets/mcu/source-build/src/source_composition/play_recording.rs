//! Finite recording call bounds from the original checked software definitions.
//! These are requested ceilings; they do not enlarge physical storage or grants.
use conduit_mcu_host_runtime::{
    execution_authority::{Operation, Refusal},
    execution_factory::SoftwareDefinition,
    native_authority,
};
pub(super) fn operation<'a>(
    definitions: &[Option<SoftwareDefinition<'_>>],
    native_kinds: &'a [(u16, &'a str)],
) -> Result<Option<Operation<'a>>, Refusal> {
    let maximum_records = definitions
        .iter()
        .flatten()
        .filter_map(|definition| match definition {
            SoftwareDefinition::RecordingLog { maximum_samples } => maximum_samples.checked_add(
                conduit_environment_runtime::recording::LOG_CONTEXT_BYTES
                    .div_ceil(conduit_environment_runtime::recording::LOG_CONTEXT_PART_BYTES)
                    as u32,
            ),
            _ => None,
        })
        .max();
    native_authority::append_operation(native_kinds, maximum_records)
}
