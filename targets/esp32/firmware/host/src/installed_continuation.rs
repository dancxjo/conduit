//! Boot-only actual NOR read into separately funded immutable continuation RAM.
use conduit_mcu_host_runtime::continuation_artifact::PREFIX_BYTES;
use conduit_resource_snapshot::nor::{NorRefusal, NorStorage};

/// Before Equipment consumes the sole physical storage owner, read the reserved
/// prefix into caller-funded RAM. No write, erase or allocation is performed.
/// Caller funds this full 8 KiB before cold preparation and retains it unchanged
/// while intent/scope/Play borrows it. Canonical decode scratch is separate and
/// transient. Root verifies these actual bytes and all signed installed facts;
/// this physical read by itself provides no membership or execution authority.
pub fn cold_read(
    storage: &mut crate::equipment::RecordingStorage,
    output: &mut [u8; PREFIX_BYTES],
) -> Result<(), NorRefusal> {
    let prefix = u32::try_from(PREFIX_BYTES).map_err(|_| NorRefusal::Geometry)?;
    if crate::config::RECORDING_PREFIX_BYTES < prefix
        || crate::config::RECORDING_PREFIX_BYTES > storage.capacity()
    {
        return Err(NorRefusal::Geometry);
    }
    storage.read(0, output)
}
