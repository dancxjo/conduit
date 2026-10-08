//! This image's admitted text/keymap operations require no dynamic allocation.
use core::alloc::{GlobalAlloc, Layout};
struct NoAllocation;
unsafe impl GlobalAlloc for NoAllocation {
    unsafe fn alloc(&self, _: Layout) -> *mut u8 {
        core::ptr::null_mut()
    }
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
#[global_allocator]
static ALLOCATOR: NoAllocation = NoAllocation;

// The prebuilt IA-32 Linux alloc archive retains this link reference even
// under panic=abort. This freestanding image cannot unwind across its gate.
#[cfg(all(target_arch = "x86", target_os = "linux"))]
#[unsafe(no_mangle)]
extern "C" fn rust_eh_personality() {}
