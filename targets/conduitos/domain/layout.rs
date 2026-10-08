//! Mechanism-owned finite private layout, shared by the image and its backends.
pub const STACK_BYTES: usize = 32 * 1024;
pub const RETAINED_BYTES: usize = 16 * 1024;
pub const STACK_PAGE: usize = 32;
pub const RETAINED_PAGE: usize = 48;
#[cfg(target_arch = "x86")]
pub const RETAINED_ADDRESS: usize = 0x4003_0000;
#[cfg(not(target_arch = "x86"))]
pub const RETAINED_ADDRESS: usize = 0x0043_0000;
