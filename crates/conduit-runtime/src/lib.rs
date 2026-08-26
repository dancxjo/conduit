//! Exact Plan-to-kernel lowering for every current Host.
//!
//! Conduit v1 has one execution kernel. The former hosted compatibility
//! executor and its fixture-only scheduler have been removed rather than kept
//! as an alternate feature-selected runtime.

#![no_std]

extern crate alloc;

pub mod lowering;
