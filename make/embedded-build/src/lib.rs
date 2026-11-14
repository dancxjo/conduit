//! Host-only generation of fixed Rust tables from the current Conduit plan seam.
//!
//! This crate accepts one already validated [`conduit_core::PlanFragment`] and
//! its current [`conduit_plan_lowering::lowering::LoweredPlanFragment`]. It performs
//! no Source parsing, planning, capability selection, firmware work, transport,
//! or trigger. The opt-in computation profile validates canonical program metadata
//! under explicit artifact ceilings. Unsupported facts fail closed.
//!
//! Fabrication may also supply an already checked document, an explicit profile
//! catalog and selected software definitions. Those helpers preserve ordinary
//! expansion and offer identities; they install no default application catalog
//! and establish no initialized provider, resource grant or execution authority.

mod checked_workset;
mod offer_image;
mod selected_software;
pub use checked_workset::{prepare_checked_workset, CheckedWorksetBounds};
pub use selected_software::{
    prepare_selected_software, PreparedSoftwareOffer, SelectedSoftwareBounds, SelectedSoftwareOffer,
};
mod resident_back;
mod resident_source;
pub use resident_source::{generate_source_resident_library, prepare_source_resident_library};
mod resident_binding;
mod resident_expression;
mod resident_library;
mod resident_library_borrow;
#[cfg(test)]
mod resident_library_empty_tests;
mod resident_offer;
pub use offer_image::{generate_capability_offer_image, CapabilityOfferImageError};
pub use resident_expression::{
    bind_current_mixed_resident_expressions, bind_current_resident_expressions,
    BoundResidentExpression, PreparedResidentExpression, ResidentExecutionStorage,
    ResidentExpressionBounds, ResidentExpressionRefusal, RESIDENT_EXPRESSION_IMPLEMENTATION,
    RESIDENT_EXPRESSION_PROFILE,
};
pub use resident_library::generate_resident_library;
mod assigned_layout;
mod assigned_native;
mod assigned_plan;
mod configuration;
mod configuration_record;
pub use configuration_record::encode_assigned_software_configuration;
mod fore;
mod line_fore;
pub use line_fore::{generate_line_fore_native_plan, GeneratedLineForePlan, SelectedForeSession};
mod generate;
pub use fore::{
    assigned_fore_inventory, generate_fore_native_plan, GeneratedForePlan, MAXIMUM_LOCAL_FORE_PORTS,
};
mod model;
mod render;
mod validate;

pub use assigned_layout::{
    assigned_layout_requirements, assigned_native_layout_requirements,
    assigned_resident_native_layout_requirements,
};
pub use assigned_plan::{
    encode_assigned_kernel_plan, encode_assigned_native_plan, encode_assigned_plan,
    encode_assigned_resident_native_plan,
};
pub use configuration_record::assigned_computation_requirements;
pub use generate::{generate_embedded_plan, generate_embedded_plan_with_computations};
pub use model::*;

pub const GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION: u32 = 0;

#[cfg(test)]
mod expression_configuration_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod renderer_tests;

#[cfg(test)]
mod resident_expression_tests;
#[cfg(test)]
mod resident_instance_tests;

mod body_fragment;
pub use body_fragment::*;

#[cfg(test)]
mod resident_terminal_tests;
