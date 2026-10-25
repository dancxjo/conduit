//! Nano boot prerequisites; no board execution is claimed by these modules.
pub mod display;
pub mod display_boot;
#[cfg(target_arch = "aarch64")]
pub mod display_mmio;
pub mod dtb;
pub mod entry;
pub mod firmware_reservations;
pub mod handoff;
#[cfg(test)]
mod handoff_tests;
pub mod memory;

#[cfg(test)]
mod dtb_tests;

#[cfg(all(target_arch = "aarch64", feature = "aarch64-jetson-nano"))]
pub mod machine;
