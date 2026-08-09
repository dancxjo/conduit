//! Host-only generation of fixed Rust tables from the current Conduit plan seam.
//!
//! This crate accepts one already validated [`conduit_core::PlanFragment`] and
//! its current [`conduit_runtime::lowering::LoweredPlanFragment`]. It performs
//! no parsing, planning, capability selection, firmware work, transport, or
//! trigger. Unsupported facts fail closed rather than being approximated.

mod generate;
mod model;
mod render;
mod validate;

pub use generate::generate_embedded_plan;
pub use model::*;

pub const GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION: u32 = 0;

#[cfg(test)]
mod tests;
