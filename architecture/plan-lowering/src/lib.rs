//! Exact plan-to-kernel lowering for every current host.
//!
//! This package owns only the pre-play rich-identity to numeric-table boundary
//! and fixed kernel storage profiles. It does not own product lifecycle, Host
//! effects, semantic implementations, planning policy, or scheduling.

#![no_std]

extern crate alloc;

pub mod fragment_set;
pub mod lowering;
pub mod shared_pool_runtime;
