//! Admit one authoritative Store backing before decoding or publication. Leases
//! move only its pointer, including while an incremental writer owns custody.
use super::*;
pub(super) fn reserve_with(
    allocate: impl FnOnce(core::alloc::Layout) -> *mut u8,
) -> Result<Box<core::mem::MaybeUninit<Store>>, &'static str> {
    let raw = allocate(core::alloc::Layout::new::<Store>()).cast::<core::mem::MaybeUninit<Store>>();
    if raw.is_null() {
        return Err("restart-store-capacity");
    }
    // SAFETY: exact nonzero Store layout; Box::write initializes it once, and
    // every refusal drops only this uninitialized allocation.
    Ok(unsafe { Box::from_raw(raw) })
}
pub(super) fn reserve() -> Result<Box<core::mem::MaybeUninit<Store>>, &'static str> {
    reserve_with(|layout| unsafe { alloc::alloc::alloc(layout) })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocation_refusal_does_not_acquire_or_replace_store() {
        let mut attempted = false;
        assert!(reserve_with(|layout| {
            attempted = true;
            assert_eq!(layout.size(), core::mem::size_of::<Store>());
            core::ptr::null_mut()
        })
        .is_err());
        assert!(attempted);
        assert_eq!(
            core::mem::size_of::<StoreLease>(),
            core::mem::size_of::<usize>()
        );
        assert!(core::mem::size_of::<Slot>() <= 2 * core::mem::size_of::<usize>());
    }
}
