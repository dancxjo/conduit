//! Fallible cold backing placement and explicit unused-storage retirement.
use super::*;
/// Descriptive local mechanism ID only; this allocation conveys no grant.
pub(super) fn reserve_local_base_id() -> Result<c::BaseImplementationId, ()> {
    let mut id = alloc::string::String::new();
    id.try_reserve_exact(c::LOCAL_BASE_IMPLEMENTATION_ID.len())
        .map_err(|_| ())?;
    id.push_str(c::LOCAL_BASE_IMPLEMENTATION_ID);
    Ok(id.into())
}

/// Protect both exact execution allocations before temporary descriptions can
/// fragment their holes. Every byte remains inside the same admitted workspace.
pub(super) fn reserve_phased_storage(
    storage: Option<n::PreparedNetworkKernelStorage>,
    custody: &IssuedCustody,
) -> Result<(n::PreparedNetworkKernelStorage, c::BaseImplementationId), ()> {
    let storage = storage
        .map_or_else(NativePlay::reserve_linked_backing, Ok)
        .map_err(|()| trace_phased_refusal(114, custody, false, false))?;
    #[cfg(test)]
    record_phased(9, Some(storage.as_ptr() as usize));
    let local_base =
        reserve_local_base_id().map_err(|()| trace_phased_refusal(116, custody, false, false))?;
    Ok((storage, local_base))
}

impl NativePlay {
    pub(crate) fn reserve_linked_backing() -> Result<n::PreparedNetworkKernelStorage, ()> {
        #[cfg(test)]
        let _refusal = if REFUSE_NEXT_BACKING.with(|flag| flag.replace(false)) {
            Some(crate::allocation_probe::refuse_next_allocation_size(
                core::mem::size_of::<PreparedNetworkKernel>(),
            ))
        } else {
            None
        };
        #[cfg(test)]
        let _log_refusal = if REFUSE_NEXT_LOG.with(|flag| flag.replace(false)) {
            Some(crate::allocation_probe::refuse_next_allocation_size(
                n::CONTROL_SIGN_BACKING_BYTES,
            ))
        } else {
            None
        };
        n::PreparedNetworkKernelStorage::try_new_observing_failure(
            diagnostics::trace_storage_failure,
        )
        .map_err(|_| ())
    }
}
#[cfg(test)]
std::thread_local! {
    static REFUSE_NEXT_BACKING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static REFUSE_NEXT_LOG: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
pub(crate) fn refuse_next_log() {
    REFUSE_NEXT_LOG.with(|flag| flag.set(true));
}
#[cfg(test)]
pub(crate) fn refuse_next_backing() {
    REFUSE_NEXT_BACKING.with(|flag| flag.set(true));
}
pub(super) fn reserve_kernel_backing(
) -> Result<alloc::boxed::Box<core::mem::MaybeUninit<PreparedNetworkKernel>>, ()> {
    reserve_kernel_backing_using(|layout| {
        #[cfg(test)]
        if REFUSE_NEXT_BACKING.with(|flag| flag.replace(false)) {
            return core::ptr::null_mut();
        }
        // SAFETY: allocator receives the exact requested nonzero layout.
        unsafe { alloc::alloc::alloc(layout) }
    })
}
fn reserve_kernel_backing_using(
    allocate: impl FnOnce(core::alloc::Layout) -> *mut u8,
) -> Result<alloc::boxed::Box<core::mem::MaybeUninit<PreparedNetworkKernel>>, ()> {
    let layout = core::alloc::Layout::new::<PreparedNetworkKernel>();
    if layout.size() == 0 {
        return Ok(alloc::boxed::Box::<PreparedNetworkKernel>::new_uninit());
    }
    // SAFETY: exact layout, unique ownership, initialized only by Box::write.
    let raw = allocate(layout).cast::<core::mem::MaybeUninit<PreparedNetworkKernel>>();
    if raw.is_null() {
        return Err(());
    }
    Ok(unsafe { alloc::boxed::Box::from_raw(raw) })
}

#[cfg(test)]
mod backing_tests {
    use super::*;
    #[test]
    fn actual_kernel_layout_refusal_returns_without_initialized_owner() {
        let mut requested = None;
        let result = reserve_kernel_backing_using(|layout| {
            requested = Some(layout);
            core::ptr::null_mut()
        });
        assert!(result.is_err());
        assert_eq!(
            requested,
            Some(core::alloc::Layout::new::<PreparedNetworkKernel>())
        );
    }
}
