#![no_std]

extern crate alloc;

pub mod allocation;
pub mod arch;
pub mod boot;
#[cfg(any(test, target_arch = "x86_64"))]
pub mod bounded_host_operations;
pub mod composition;
pub mod display;
pub mod dual_region_composition;
pub mod dual_region_kernel;
pub mod dual_region_plan;
mod execution_region;
#[cfg(any(
    test,
    target_arch = "x86_64",
    feature = "aarch64-product",
    feature = "hosted-tools"
))]
pub mod fabrication;
#[cfg(any(test, target_arch = "x86_64", feature = "aarch64-product"))]
pub mod front_door;
pub mod functional_offers;
#[cfg(target_arch = "x86_64")]
pub mod hotplug_guest;
#[cfg(any(test, feature = "native-http-client"))]
pub mod http;
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
#[cfg(any(test, feature = "native-compositor"))]
pub mod native_compositor;
pub mod observatory;
pub mod offer;
#[cfg(any(test, target_arch = "x86_64", feature = "aarch64-product"))]
pub mod offer_fabrication;
#[cfg(any(target_arch = "x86_64", feature = "hosted-tools"))]
pub mod opl2_offer;
#[cfg(target_arch = "x86_64")]
pub mod opl2_plan;
#[cfg(target_arch = "x86_64")]
pub mod opl2_play;
mod ordinary_form;
pub mod ordinary_plan;
#[cfg(any(target_arch = "x86_64", feature = "hosted-tools"))]
#[cfg_attr(
    all(feature = "hosted-tools", not(target_arch = "x86_64")),
    allow(dead_code)
)]
pub mod pc_speaker_offer;
#[cfg(target_arch = "x86_64")]
pub mod pc_speaker_plan;
#[cfg(target_arch = "x86_64")]
pub mod pc_speaker_play;
pub mod planned_kernel;
#[path = "presentation_nucleus/offers.rs"]
mod presentation_offers;
// Product entrances remain out of A0-A4 proof appliances. AArch64 admits
// these modules only through its distinct PROFILE-selected product feature.
#[cfg(any(test, target_arch = "x86_64", feature = "aarch64-product"))]
pub mod linear_presenter;
#[cfg(any(test, target_arch = "x86_64", feature = "hosted-tools"))]
pub mod presentation_nucleus;
#[cfg(any(test, target_arch = "x86_64", feature = "aarch64-product"))]
mod product_bindings;
#[cfg(all(target_arch = "x86_64", feature = "native-compositor"))]
pub mod product_front_door;
#[cfg(any(test, target_arch = "x86_64", feature = "aarch64-product"))]
pub mod product_journey;
pub mod proof;
#[cfg(target_arch = "x86_64")]
pub mod rescue_guest;
#[cfg(any(test, target_arch = "x86_64"))]
pub mod synth_nucleus;
pub mod text_composition;
mod text_kernel_operations;
mod text_offer;
pub mod text_planned_kernel;
pub mod text_upper;
#[cfg(any(test, target_arch = "x86_64"))]
pub mod timer_nucleus;
mod timing_plan;
pub mod timing_profile;
