//! C allocation boundary for the pinned radio SDK. Native Root gives it a
//! dedicated arena; the attended bench retains its previous shared allocator.
//! C callers must free only live pointers returned here, once, and obey their
//! allocation extents. There is no fallback allocator on pressure or overflow.
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr,
};

const HEADER: usize = core::mem::size_of::<usize>();

#[cfg(feature = "native-root")]
pub const SDK_BYTES: usize = 120 * 1024;
#[cfg(feature = "native-root")]
static SDK_HEAP: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

fn heap() -> &'static esp_alloc::EspHeap {
    #[cfg(feature = "native-root")]
    {
        &SDK_HEAP
    }
    #[cfg(not(feature = "native-root"))]
    {
        &esp_alloc::HEAP
    }
}

/// Unique proof that this Boot initialized the SDK's actual isolated backing.
/// It admits storage only; Root must separately bound operations and cleanup.
#[cfg(feature = "native-root")]
pub struct NativeSdkReservation {
    _private: (),
}
#[cfg(feature = "native-root")]
impl NativeSdkReservation {
    pub fn bytes(&self) -> usize {
        SDK_HEAP.stats().size
    }
}

/// Called once before RTOS/radio startup. The total backing remains 184 KiB:
/// 64 KiB for Root Rust allocations and 32+88 KiB for the SDK C boundary.
#[cfg(feature = "native-root")]
pub fn initialize() -> NativeSdkReservation {
    use core::{
        mem::MaybeUninit,
        sync::atomic::{AtomicBool, Ordering},
    };
    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    assert!(!INITIALIZED.swap(true, Ordering::SeqCst));
    static mut FIRST: [MaybeUninit<u32>; 32 * 1024 / 4] = [MaybeUninit::uninit(); 32 * 1024 / 4];
    #[esp_hal::ram(reclaimed)]
    static mut SECOND: [MaybeUninit<u32>; 88 * 1024 / 4] = [MaybeUninit::uninit(); 88 * 1024 / 4];
    // SAFETY: uniquely initialized, disjoint internal static backing, retained
    // for this Boot. Root never adds these regions to its global allocator.
    unsafe {
        SDK_HEAP.add_region(esp_alloc::HeapRegion::new(
            ptr::addr_of_mut!(FIRST).cast(),
            32 * 1024,
            esp_alloc::MemoryCapability::Internal.into(),
        ));
        SDK_HEAP.add_region(esp_alloc::HeapRegion::new(
            ptr::addr_of_mut!(SECOND).cast(),
            88 * 1024,
            esp_alloc::MemoryCapability::Internal.into(),
        ));
    }
    assert_eq!(SDK_HEAP.stats().size, SDK_BYTES);
    assert_eq!(esp_alloc::HEAP.stats().size, 64 * 1024);
    self_check();
    NativeSdkReservation { _private: () }
}

#[cfg(feature = "native-root")]
fn self_check() {
    let root_used = esp_alloc::HEAP.used();
    // SAFETY: exercise actual C boundary before any concurrent SDK allocation.
    unsafe {
        let input = calloc(16, 1);
        assert!(!input.is_null());
        assert!(SDK_HEAP.used() >= 16 + HEADER);
        assert_eq!(esp_alloc::HEAP.used(), root_used);
        for i in 0..16 {
            assert_eq!(*input.add(i), 0);
            *input.add(i) = i as u8;
        }
        let grown = realloc_internal(input, 32);
        assert!(!grown.is_null());
        for i in 0..16 {
            assert_eq!(*grown.add(i), i as u8);
        }
        assert!(realloc_internal(grown, usize::MAX).is_null());
        assert!(malloc(SDK_BYTES).is_null());
        assert!(calloc(usize::MAX, usize::MAX).is_null());
        for i in 0..16 {
            assert_eq!(*grown.add(i), i as u8);
        }
        free(grown);
        free(ptr::null_mut());
    }
    assert_eq!(SDK_HEAP.used(), 0);
    assert_eq!(esp_alloc::HEAP.used(), root_used);
}

#[cfg(feature = "native-root")]
pub fn stats() -> esp_alloc::HeapStats {
    SDK_HEAP.stats()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    let Some(total) = size.checked_add(HEADER) else {
        return ptr::null_mut();
    };
    let Ok(layout) = Layout::from_size_align(total, HEADER) else {
        return ptr::null_mut();
    };
    let base = unsafe { heap().alloc_caps(esp_alloc::MemoryCapability::Internal.into(), layout) };
    if base.is_null() {
        return base;
    }
    unsafe {
        base.cast::<usize>().write(total);
        base.add(HEADER)
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn malloc_internal(size: usize) -> *mut u8 {
    unsafe { malloc(size) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn free(pointer: *mut u8) {
    if pointer.is_null() {
        return;
    }
    unsafe {
        let base = pointer.sub(HEADER);
        let total = base.cast::<usize>().read();
        let layout = Layout::from_size_align_unchecked(total, HEADER);
        GlobalAlloc::dealloc(heap(), base, layout);
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn free_internal(pointer: *mut u8) {
    unsafe { free(pointer) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut u8 {
    let Some(size) = count.checked_mul(size) else {
        return ptr::null_mut();
    };
    let pointer = unsafe { malloc(size) };
    if !pointer.is_null() {
        unsafe {
            pointer.write_bytes(0, size);
        }
    }
    pointer
}

#[unsafe(no_mangle)]
unsafe extern "C" fn calloc_internal(count: usize, size: usize) -> *mut u8 {
    unsafe { calloc(count, size) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn realloc(pointer: *mut u8, size: usize) -> *mut u8 {
    if size == 0 {
        unsafe {
            free(pointer);
        }
        return ptr::null_mut();
    }
    let replacement = unsafe { malloc(size) };
    if !replacement.is_null() && !pointer.is_null() {
        unsafe {
            let old_size = pointer.sub(HEADER).cast::<usize>().read() - HEADER;
            ptr::copy_nonoverlapping(pointer, replacement, old_size.min(size));
            free(pointer);
        }
    }
    replacement
}

#[unsafe(no_mangle)]
unsafe extern "C" fn realloc_internal(pointer: *mut u8, size: usize) -> *mut u8 {
    unsafe { realloc(pointer, size) }
}

#[unsafe(no_mangle)]
extern "C" fn get_free_internal_heap_size() -> usize {
    heap().free()
}
