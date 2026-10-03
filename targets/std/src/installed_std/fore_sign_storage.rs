//! Pre-Play storage for retained external Fore correlation history.
use conduit_kernel::{HostedSignLog, KernelEvent};

pub(super) fn prepare(event_items: u16, has_fore: bool) -> Result<HostedSignLog, String> {
    let event_bytes = u32::from(event_items)
        .checked_mul(
            u32::try_from(core::mem::size_of::<KernelEvent>())
                .map_err(|_| "installed Sign charge overflow")?,
        )
        .ok_or("installed Sign byte budget overflow")?;
    // Queue slots bound simultaneous payloads, not lifetime history. Every
    // remote correlation owns one retained (non-transient) kernel event, so
    // the admitted event ceiling also bounds all remote correlation entries.
    // The kernel preserves those events when transient history is evicted.
    let remote_items = if has_fore { event_items } else { 0 };
    let remote_bytes = conduit_kernel::remote_sign_storage_bytes(remote_items)
        .ok_or("external Fore remote Sign byte budget overflow")?;
    HostedSignLog::new_with_remote_storage(event_items, event_bytes, remote_items, remote_bytes)
        .map_err(|error| format!("installed Sign store: {error:?}"))
}
