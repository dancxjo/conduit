//! Nano boot prerequisites; no board execution is claimed by these modules.
pub mod dtb;
pub mod memory;

#[cfg(test)]
mod dtb_tests;
