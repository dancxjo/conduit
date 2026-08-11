#![no_std]

extern crate alloc;

pub mod allocation;
pub mod arch;
pub mod boot;
pub mod composition;
pub mod dual_region_composition;
pub mod dual_region_kernel;
pub mod dual_region_plan;
mod execution_region;
pub mod identity;
#[cfg(test)]
pub mod kernel_profile;
pub mod keyboard_bridge;
pub mod keyboard_offer;
pub mod keyboard_plan;
pub mod keyboard_play;
pub mod machine;
pub mod machine_a2_kernel;
pub mod observatory;
pub mod offer;
mod ordinary_form;
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
