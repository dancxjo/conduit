//! Exact computational contracts and shared kernel Backs, independent of the
//! domain catalogue. Construction does not select providers or grant authority.
#![no_std]
extern crate alloc;

mod pure_expression;
mod structured_selector;
pub use pure_expression::{pure_expression_contract, pure_filter_contract};
pub use structured_selector::{structured_selector_contract, STRUCTURED_SELECTOR_REVISION};

#[cfg(feature = "kernel-step")]
mod prepared_expression_back;
#[cfg(feature = "kernel-step")]
mod structured_selector_back;
#[cfg(feature = "kernel-step")]
pub use prepared_expression_back::{prepared_expression_bounds, PreparedExpressionBack};
#[cfg(feature = "kernel-step")]
pub use structured_selector_back::StructuredSelectorBack;
