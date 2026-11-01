//! Fallible affine private-owner backing. Refusal drops the untransferred value,
//! including its existing secret-erasing field destructors.
use alloc::boxed::Box;
use core::alloc::Layout;

pub(super) fn try_box<T>(value: T) -> Result<Box<T>, ()> {
    // SAFETY: the global allocator fulfills the exact nonzero layout contract.
    unsafe {
        try_box_with(value, |layout| {
            // SAFETY: the caller below handles null and uses the exact same layout.
            alloc::alloc::alloc(layout)
        })
    }
}

/// # Safety
/// A non-null allocator result must be uniquely owned global-allocator storage
/// for the exact requested layout; null is always permitted.
pub(crate) unsafe fn try_box_with<T>(
    value: T,
    allocate: impl FnOnce(Layout) -> *mut u8,
) -> Result<Box<T>, ()> {
    let layout = Layout::new::<T>();
    if layout.size() == 0 {
        return Ok(Box::new(value));
    }
    let raw = allocate(layout).cast::<T>();
    if raw.is_null() {
        return Err(());
    }
    // SAFETY: allocator supplies unique storage for T's exact layout. Ownership
    // moves once; Box will run the original destructor and free the same layout.
    unsafe {
        raw.write(value);
        Ok(Box::from_raw(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    struct PrivateValue<'a> {
        drops: &'a Cell<usize>,
        bytes: [u8; 128],
    }
    impl Drop for PrivateValue<'_> {
        fn drop(&mut self) {
            for byte in &mut self.bytes {
                // SAFETY: uniquely owned private bytes, matching secret owners.
                unsafe { core::ptr::write_volatile(byte, 0) };
            }
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[test]
    fn null_refuses_and_drops_untransferred_private_value_once() {
        let drops = Cell::new(0);
        let value = PrivateValue {
            drops: &drops,
            bytes: [42; 128],
        };
        // SAFETY: null fulfills the allocator refusal contract.
        let result = unsafe {
            try_box_with(value, |layout| {
                assert_eq!(layout, Layout::new::<PrivateValue<'_>>());
                core::ptr::null_mut()
            })
        };
        assert!(result.is_err());
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn successful_backing_preserves_value_and_unique_destructor() {
        let drops = Cell::new(0);
        let owned = try_box(PrivateValue {
            drops: &drops,
            bytes: [42; 128],
        })
        .unwrap();
        assert_eq!(owned.bytes, [42; 128]);
        assert_eq!(drops.get(), 0);
        drop(owned);
        assert_eq!(drops.get(), 1);
    }
}
