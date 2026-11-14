//! Boot-only immutable view of the reserved continuation prefix. No authority.
use conduit_mcu_host_runtime::continuation_artifact::PREFIX_BYTES;
use conduit_resource_snapshot::nor::NorRefusal;
use conduit_rp2040_flash_layout::DURABLE;

/// Read physically installed bytes without retaining another 8 KiB RAM copy.
/// Root must independently verify the accepted anchor, owner intent and pins.
/// Merely obtaining this view never restores membership or Starts a Play.
///
/// # Safety
/// Call before Equipment or any flash/electrical actor starts, with the actual
/// original initialized FLASH owner. The caller must preserve these exact first
/// PREFIX_BYTES for the lifetime of the returned reference: no program or erase,
/// including live installation, may ever touch them. Normal recording must use
/// the original configured prefix exclusion (checked here and again by its
/// residence owner). Firmware/ROM maintenance that replaces the prefix requires
/// a reset and termination of all borrowers first. Geometry alone is no witness
/// of this lifetime exclusion or of media authenticity.
pub unsafe fn cold_prefix(
    _flash: &crate::credential_source::Device,
) -> Result<&'static [u8; PREFIX_BYTES], NorRefusal> {
    let prefix = u32::try_from(PREFIX_BYTES).map_err(|_| NorRefusal::Geometry)?;
    if crate::config::RECORDING_PREFIX_BYTES < prefix
        || crate::config::RECORDING_PREFIX_BYTES > DURABLE.bytes()
        || !DURABLE.contains(DURABLE.offset(), prefix)
    {
        return Err(NorRefusal::Geometry);
    }
    // SAFETY: checked target XIP geometry plus the caller's actual cold FLASH
    // possession and lifetime exclusion above; all byte patterns are valid u8.
    Ok(unsafe { &*(DURABLE.address() as *const [u8; PREFIX_BYTES]) })
}

/// Bounded cold decode/verification scratch retires before Source preparation.
/// The result is original Root-verified accepted state, never a fresh receipt.
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
