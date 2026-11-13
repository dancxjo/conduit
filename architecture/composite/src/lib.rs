//! Kernel-backed execution for exact composite definitions.

#![no_std]

#[macro_use]
extern crate alloc;
#[cfg(test)]
extern crate std;

mod prelude {
    pub use alloc::boxed::Box;
    #[cfg(feature = "kernel-executor")]
    pub use alloc::string::{String, ToString};
    pub use alloc::vec::Vec;
}

#[cfg(feature = "kernel-executor")]
mod boundary;
#[cfg(feature = "kernel-executor")]
mod bounded_activation;
#[cfg(feature = "kernel-executor")]
mod bounded_fold_activation;
#[cfg(feature = "kernel-executor")]
mod bounded_scan_activation;
#[cfg(feature = "kernel-executor")]
mod canonical_export;
#[cfg(feature = "kernel-executor")]
mod child;
#[cfg(feature = "kernel-executor")]
mod region_kernel;
#[cfg(feature = "kernel-executor")]
pub use child::{ChildExecutionError, ChildTerminalError, ChildTransportError};
#[cfg(feature = "kernel-executor")]
pub use region_kernel::{PreparedRegionKernel, RegionMemoryReceipt};
mod current_sample;
#[cfg(feature = "kernel-executor")]
mod definition;
mod flow_singleton;
pub use flow_singleton::FlowSingletonBack;
mod flow_concat_finite;
pub use flow_concat_finite::FlowConcatFiniteBack;
mod flow_merge_finite;
mod value_backing;
#[cfg(feature = "kernel-executor")]
mod flow_select;
mod flow_zip;
mod value_pair;
pub use value_pair::ValuePairBack;
#[cfg(feature = "kernel-executor")]
mod kernel_executor;
#[cfg(feature = "kernel-executor")]
mod operation;
#[cfg(feature = "kernel-executor")]
mod planned_activation;
#[cfg(feature = "kernel-executor")]
mod reusable_value_pool;
mod seeded_state;
#[cfg(test)]
mod test_support;

#[cfg(feature = "kernel-executor")]
pub use bounded_activation::*;
#[cfg(feature = "kernel-executor")]
pub use bounded_fold_activation::*;
#[cfg(feature = "kernel-executor")]
pub use bounded_scan_activation::*;
pub use current_sample::CurrentSampleBack;
#[cfg(feature = "kernel-executor")]
pub use definition::*;
pub use flow_merge_finite::FlowMergeFiniteBack;
#[cfg(feature = "kernel-executor")]
pub use flow_select::*;
pub use flow_zip::{FlowZipBack, FlowZipExactBack};
#[cfg(feature = "kernel-executor")]
pub use kernel_executor::*;
#[cfg(feature = "kernel-executor")]
pub use operation::*;
#[cfg(feature = "kernel-executor")]
pub use planned_activation::*;
pub use seeded_state::SeededStateBack;

#[cfg(feature = "kernel-executor")]
mod kernel_each_back;
#[cfg(feature = "kernel-executor")]
mod kernel_scan_back;
#[cfg(feature = "kernel-executor")]
pub use kernel_each_back::KernelEachBack;
#[cfg(feature = "kernel-executor")]
pub use kernel_scan_back::KernelScanBack;
