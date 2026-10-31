//! Compiler-only linked descriptions shared by Host and Body Make.
//!
//! Emission preserves complete immutable metadata. Generated construction does
//! not establish a current Base, Host/Boot, capability possession or execution.
mod configuration_value;
mod offer;
mod rust_expression;

pub use configuration_value::emit as emit_configuration_value;
pub use offer::write_offer;
pub use rust_expression::emit as emit_rust_expression;
