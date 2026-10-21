//! The exact admitted allocator is shared with the actual-grant regression.
pub fn exhausted() -> bool {
    // SAFETY: service dispatch/panic runs only with its fixed grant installed.
    unsafe { conduitos_application_core::private_heap::exhausted() }
}
#[global_allocator]
static HEAP: conduitos_application_core::private_heap::PrivateHeap =
    conduitos_application_core::private_heap::PrivateHeap;
