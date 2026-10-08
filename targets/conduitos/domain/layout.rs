//! Mechanism-owned finite private layout, shared by the image and its backends.
pub const STACK_BYTES: usize = 32 * 1024;
pub const RETAINED_BYTES: usize = 16 * 1024;
pub const STACK_PAGE: usize = 64;
pub const RETAINED_PAGE: usize = 96;
#[cfg(target_arch = "x86")]
#[cfg(conduitos_domain_image)]
pub const RETAINED_ADDRESS: usize = 0x4006_0000;
#[cfg(not(target_arch = "x86"))]
#[cfg(conduitos_domain_image)]
pub const RETAINED_ADDRESS: usize = 0x0046_0000;
