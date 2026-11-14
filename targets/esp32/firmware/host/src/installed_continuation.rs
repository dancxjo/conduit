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

/// Original retained IMAGE-heap backing, never an XIP alias or received DTO.
pub struct OwnedPrefix(alloc::vec::Vec<u8>);
impl OwnedPrefix {
    pub fn bytes(&self) -> &[u8; PREFIX_BYTES] {
        self.0
            .as_slice()
            .try_into()
            .expect("fixed physically read prefix")
    }
}
/// Allocate only the declared prefix bytes, before physical Equipment custody
/// moves. A pristine erased prefix has no installed intent and retains no heap.
pub fn read_owned(
    storage: &mut crate::equipment::RecordingStorage,
) -> Result<Option<OwnedPrefix>, NorRefusal> {
    let mut bytes = alloc::vec::Vec::new();
    bytes
        .try_reserve_exact(PREFIX_BYTES)
        .map_err(|_| NorRefusal::Capacity)?;
    bytes.resize(PREFIX_BYTES, 0);
    cold_read(
        storage,
        bytes
            .as_mut_slice()
            .try_into()
            .map_err(|_| NorRefusal::Geometry)?,
    )?;
    if bytes.iter().all(|byte| *byte == 0xff) {
        Ok(None)
    } else {
        Ok(Some(OwnedPrefix(bytes)))
    }
}
/// Root checks physically read bytes before Source receives any installed state.
/// This separately funded scratch is transient and drops before local Play.
pub fn accept<const N: usize>(
    root: &mut conduit_mcu_host_runtime::root::Root<'_, N>,
    prefix: &[u8; PREFIX_BYTES],
) -> Result<
    conduit_mcu_host_runtime::root::InstalledContinuationAcceptance,
    conduit_mcu_host_runtime::root::ContinuationAcceptanceRefusal,
> {
    use conduit_mcu_host_runtime::root::ContinuationAcceptanceRefusal as E;
    let mut scratch = alloc::vec::Vec::new();
    scratch
        .try_reserve_exact(PREFIX_BYTES)
        .map_err(|_| E::Bounds)?;
    scratch.resize(PREFIX_BYTES, 0);
    let scratch: &mut [u8; PREFIX_BYTES] =
        scratch.as_mut_slice().try_into().map_err(|_| E::Bounds)?;
    let accepted = root.read_continuation_acceptance(prefix, scratch)?;
    root.restore_continuation_acceptance(&accepted)?;
    Ok(accepted)
}
