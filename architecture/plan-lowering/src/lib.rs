//! Exact plan-to-kernel lowering for every current host.
//!
//! This package owns only the pre-play rich-identity to numeric-table boundary
//! and fixed kernel storage profiles. It does not own product lifecycle, Host
//! effects, semantic implementations, planning policy, or scheduling.

#![no_std]

extern crate alloc;
mod active_scope;
pub mod assigned_play_scope;
#[cfg(feature = "body-plan-binding")]
pub mod body_fragment_scope;

#[cfg(feature = "delegated-preparation")]
pub mod delegated_kernel;

pub mod activation_fragment;
pub mod execution_regions;
pub mod fragment_set;
pub mod lowering;
pub mod shared_pool_runtime;

/// Exact single-call native selection validation before Play.
pub mod selected_operation;
pub mod selected_operation_set;

pub mod resident_inventory;
pub mod selected_assigned_expression;
/// Exact selected native operation from the constrained assigned projection.
pub mod selected_assigned_operation;
