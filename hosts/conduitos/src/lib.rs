#![no_std]

extern crate alloc;

pub mod allocation;
pub mod arch;
pub mod boot;
pub mod composition;
pub mod display;
pub mod dual_region_composition;
pub mod dual_region_kernel;
pub mod dual_region_plan;
mod execution_region;
#[cfg(target_arch = "x86_64")]
pub mod hotplug_guest;
pub mod identity;
#[cfg(test)]
pub mod kernel_profile;
pub mod keyboard_bridge;
#[cfg(target_arch = "x86_64")]
pub mod keyboard_input;
pub mod keyboard_offer;
pub mod keyboard_plan;
pub mod keyboard_play;
#[cfg(target_arch = "x86_64")]
pub mod keyboard_text_guest;
pub mod keyboard_text_observatory;
mod keyboard_text_operations;
pub mod keyboard_text_plan;
pub mod keyboard_text_play;
#[cfg(test)]
mod keyboard_text_play_tests;
pub mod local_rescue;
pub mod machine;
pub mod machine_a2_kernel;
pub mod observatory;
pub mod offer;
#[cfg(target_arch = "x86_64")]
pub mod opl2_offer;
#[cfg(target_arch = "x86_64")]
pub mod opl2_plan;
#[cfg(target_arch = "x86_64")]
pub mod opl2_play;
mod ordinary_form;
pub mod ordinary_plan;
#[cfg(target_arch = "x86_64")]
pub mod pc_speaker_offer;
#[cfg(target_arch = "x86_64")]
pub mod pc_speaker_plan;
#[cfg(target_arch = "x86_64")]
pub mod pc_speaker_play;
pub mod planned_kernel;
pub mod presentation_nucleus;
pub mod proof;
#[cfg(target_arch = "x86_64")]
pub mod rescue_guest;
pub mod text_composition;
mod text_kernel_operations;
mod text_offer;
pub mod text_planned_kernel;
pub mod text_upper;
mod timing_plan;
pub mod timing_profile;
