// Finite recording call bounds from the original checked software definitions.
// These are requested ceilings; they do not enlarge physical storage or grants.
use conduit_mcu_host_runtime::{
    execution_authority::{Issuer, Operation, Refusal},
    execution_factory::SoftwareDefinition,
    native_authority,
};
#[cfg(not(record_service))]
pub type RecordingOperation<'a> = Operation<'a>;
#[cfg(record_service)]
pub type RecordingOperation<'a> = [Operation<'a>; 2];
#[cfg(not(record_service))]
pub type RecordingIssuers<'a> = &'a mut Issuer;
#[cfg(record_service)]
pub type RecordingIssuers<'a> = [&'a mut Issuer; 2];

#[cfg(not(record_service))]
pub const OUTPUT_BYTES: usize = conduit_resource_snapshot::native_append::APPEND_RESULT_BYTES as usize;
#[cfg(record_service)]
pub const OUTPUT_BYTES: usize = conduit_resource_snapshot::read_result::READ_RESULT_BYTES;

pub(super) fn operation<'a>(
    definitions: &[Option<SoftwareDefinition<'_>>],
    native_kinds: &'a [(u16, &'a str)],
) -> Result<Option<RecordingOperation<'a>>, Refusal> {
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
    #[cfg(not(record_service))]
    { native_authority::append_operation(native_kinds, maximum_records) }
    #[cfg(record_service)]
    { native_authority::record_service_operations(native_kinds, maximum_records, crate::config::RECORDING_READ_OPERATIONS) }
}
