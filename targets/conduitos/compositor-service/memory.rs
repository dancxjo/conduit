//! Fixed private grants from ServiceMemoryLayout::compositor(). Not ELF storage.
use conduitos_compositor_core::{Compositor, MAX_PIXELS};
use core::{
    mem::{MaybeUninit, align_of, size_of},
    ptr, slice,
};

pub const EXCHANGE: usize = 0x0060_1000;
pub const EXCHANGE_BYTES: usize = 4096;
pub const OUTPUT: usize = 0x0060_3000;
pub const OUTPUT_BYTES: usize = 64 * 1024;
const METADATA: usize = 0x0062_5000;
const METADATA_BYTES: usize = 256 * 1024;
const PIXELS: usize = 0x0066_6000;
const SCRATCH: usize = 0x00e6_7000;
const INITIALIZED: u64 = 0x434f_4d50_5354_4154;

// These Rust-layout bytes never cross an ABI or leave this guest image.
#[repr(C)]
struct Retained {
    initialized: u64,
    instance: u64,
    last_request: u64,
    compositor: MaybeUninit<Compositor<'static>>,
}
const _: () = assert!(size_of::<Retained>() <= METADATA_BYTES);
const _: () = assert!(METADATA % align_of::<Retained>() == 0);
const _: () = assert!(MAX_PIXELS * size_of::<u32>() == 8 * 1024 * 1024);

#[derive(Clone, Copy, Debug)]
pub enum MemoryError {
    NotInitialized,
    AlreadyInitialized,
    WrongInstance,
    StaleRequest,
    Dimensions,
}

/// SAFETY: Root installed and zeroed the exact private grants, enters one service
/// exclusively, and never mutates metadata/pixels/scratch until quarantining it.
/// No references to those grants may be created outside this module.
pub unsafe fn initialize(
    instance: u64,
    request: u64,
    width: u16,
    height: u16,
) -> Result<(), MemoryError> {
    let state = METADATA as *mut Retained;
    // Read only an integer header: the rest initially contains no valid Rust value.
    if unsafe { ptr::addr_of!((*state).initialized).read() } != 0 {
        return Err(MemoryError::AlreadyInitialized);
    }
    if instance == 0 || request == 0 || width == 0 || height == 0 {
        return Err(MemoryError::Dimensions);
    }
    // Exclusively borrow each entire arena ONCE. Compositor swaps these borrows
    // on upload commit; reconstructing them on re-entry would alias live state.
    let pixels = unsafe { slice::from_raw_parts_mut(PIXELS as *mut u32, MAX_PIXELS) };
    let scratch = unsafe { slice::from_raw_parts_mut(SCRATCH as *mut u32, MAX_PIXELS) };
    let core =
        Compositor::new(pixels, scratch, width, height).map_err(|_| MemoryError::Dimensions)?;
    unsafe {
        ptr::addr_of_mut!((*state).compositor).write(MaybeUninit::new(core));
        ptr::addr_of_mut!((*state).instance).write(instance);
        ptr::addr_of_mut!((*state).last_request).write(request);
        // Publish last. Faulted initialization causes full Root quarantine.
        ptr::addr_of_mut!((*state).initialized).write(INITIALIZED);
    }
    Ok(())
}

/// SAFETY: same mapping/exclusivity contract as initialize. The closure cannot
/// retain the temporary borrow. Only successful session/sequence checks can act.
pub unsafe fn with_core<T>(
    instance: u64,
    request: u64,
    action: impl FnOnce(&mut Compositor<'static>) -> T,
) -> Result<T, MemoryError> {
    let state = METADATA as *mut Retained;
    if unsafe { ptr::addr_of!((*state).initialized).read() } != INITIALIZED {
        return Err(MemoryError::NotInitialized);
    }
    if unsafe { ptr::addr_of!((*state).instance).read() } != instance {
        return Err(MemoryError::WrongInstance);
    }
    if request == 0 || request <= unsafe { ptr::addr_of!((*state).last_request).read() } {
        return Err(MemoryError::StaleRequest);
    }
    // Consume admitted request identity even if the operation refuses. No replay.
    unsafe {
        ptr::addr_of_mut!((*state).last_request).write(request);
    }
    let core =
        unsafe { &mut *ptr::addr_of_mut!((*state).compositor).cast::<Compositor<'static>>() };
    Ok(action(core))
}
