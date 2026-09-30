//! Kernel-backed execution for exact composite definitions.

mod boundary;
mod bounded_activation;
mod child;
mod definition;
mod flow_select;
mod kernel_executor;
mod operation;

pub use bounded_activation::*;
pub use definition::*;
pub use flow_select::*;
pub use kernel_executor::*;
pub use operation::*;
