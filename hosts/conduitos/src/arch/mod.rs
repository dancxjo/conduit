//! Narrow bootstrap-only machine operations.
//!
//! Shared ConduitOS code depends on this surface. Architecture facts never flow
//! upward into authored meaning or semantic implementation identity.

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::{boot_entropy, deterministic_exit, early_write};

#[cfg(target_arch = "x86_64")]
pub const ARCHITECTURE: &str = "x86_64";
#[cfg(target_arch = "x86")]
pub const ARCHITECTURE: &str = "ia32";
#[cfg(target_arch = "aarch64")]
pub const ARCHITECTURE: &str = "aarch64";
#[cfg(target_arch = "riscv64")]
pub const ARCHITECTURE: &str = "riscv64";
#[cfg(target_arch = "loongarch64")]
pub const ARCHITECTURE: &str = "loongarch64";
