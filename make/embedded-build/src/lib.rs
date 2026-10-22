//! Host-only generation of fixed Rust tables from the current Conduit plan seam.
//!
//! This crate accepts one already validated [`conduit_core::PlanFragment`] and
//! its current [`conduit_plan_lowering::lowering::LoweredPlanFragment`]. It performs
//! no Source parsing, planning, capability selection, firmware work, transport,
//! or trigger. The opt-in computation profile validates canonical program metadata
//! under explicit artifact ceilings. Unsupported facts fail closed.

mod assigned_native;
mod assigned_plan;
mod configuration;
mod generate;
mod model;
mod render;
mod validate;

pub use assigned_plan::{
    assigned_computation_requirements, assigned_layout_requirements,
    assigned_native_layout_requirements, encode_assigned_kernel_plan, encode_assigned_native_plan,
    encode_assigned_plan,
};
pub use generate::{generate_embedded_plan, generate_embedded_plan_with_computations};
pub use model::*;

pub const GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION: u32 = 0;

#[cfg(test)]
mod expression_configuration_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod renderer_tests;
