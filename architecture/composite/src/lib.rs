//! Kernel-backed execution for exact composite definitions.

#![no_std]

#[macro_use]
extern crate alloc;
#[cfg(test)]
extern crate std;

mod prelude {
    pub use alloc::boxed::Box;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec::Vec;
}

mod boundary;
mod bounded_activation;
mod bounded_fold_activation;
mod bounded_scan_activation;
mod child;
pub use child::{ChildExecutionError, ChildTerminalError, ChildTransportError};
mod definition;
mod flow_select;
mod kernel_executor;
mod operation;
mod planned_activation;

pub use bounded_activation::*;
pub use bounded_fold_activation::*;
pub use bounded_scan_activation::*;
pub use definition::*;
pub use flow_select::*;
pub use kernel_executor::*;
pub use operation::*;
pub use planned_activation::*;
