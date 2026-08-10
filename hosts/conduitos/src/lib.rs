#![no_std]

extern crate alloc;

pub mod allocation;
pub mod arch;
pub mod boot;
pub mod composition;
mod execution_region;
pub mod identity;
#[cfg(test)]
pub mod kernel_profile;
pub mod machine;
pub mod observatory;
pub mod offer;
pub mod ordinary_plan;
pub mod planned_kernel;
pub mod proof;
pub mod text_composition;
mod text_kernel_operations;
mod text_offer;
pub mod text_planned_kernel;
pub mod text_upper;
mod timing_plan;
pub mod timing_profile;
