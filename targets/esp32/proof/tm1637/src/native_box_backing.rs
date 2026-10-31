//! Policy-free fallible exact-layout backing, with no initialized input owner.
use alloc::boxed::Box;
use core::mem::MaybeUninit;

pub(crate) fn reserve_box<T>() -> Result<Box<MaybeUninit<T>>, ()> {
    let layout = core::alloc::Layout::new::<T>();
    if layout.size() == 0 {
        return Ok(Box::<T>::new_uninit());
    }
    // SAFETY: exact nonzero layout; null is handled before ownership is made.
    let raw = unsafe { alloc::alloc::alloc(layout) }.cast::<MaybeUninit<T>>();
    if raw.is_null() {
        return Err(());
    }
    // SAFETY: unique exact-layout backing, never read before Box::write.
    Ok(unsafe { Box::from_raw(raw) })
}
