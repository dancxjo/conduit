#![no_std]

extern crate alloc;

pub mod allocation;
pub mod arch;
pub mod boot;
#[cfg(any(test, target_arch = "x86_64"))]
pub mod bounded_host_operations;
pub mod composition;
pub mod cooperative_timer_lane;
pub mod cryptographic_entropy;
pub mod display;
pub mod dual_region_composition;
pub mod dual_region_kernel;
pub mod dual_region_plan;
mod execution_region;
#[cfg(any(
    test,
    target_arch = "x86_64",
    feature = "ia32-product",
    feature = "aarch64-product",
    feature = "riscv64-product",
    feature = "loongarch64-product",
    feature = "aarch64-orange-pi-5",
    feature = "hosted-tools"
))]
pub mod fabrication;
#[cfg(any(
    test,
    target_arch = "x86_64",
    feature = "ia32-product",
    feature = "aarch64-product",
    feature = "riscv64-product",
    feature = "loongarch64-product",
    feature = "aarch64-orange-pi-5"
))]
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
mod native_components;
#[cfg(any(test, feature = "native-compositor"))]
pub mod native_compositor;
pub mod native_workset;
pub mod observatory;
pub mod offer;
#[cfg(any(
    test,
    target_arch = "x86_64",
    feature = "ia32-product",
    feature = "aarch64-product",
    feature = "riscv64-product",
    feature = "loongarch64-product",
    feature = "aarch64-orange-pi-5"
))]
pub mod offer_fabrication;
#[cfg(any(target_arch = "x86_64", feature = "hosted-tools"))]
pub mod opl2_offer;
#[cfg(target_arch = "x86_64")]
pub mod opl2_plan;
#[cfg(target_arch = "x86_64")]
pub mod opl2_play;
mod ordinary_form;
pub mod ordinary_plan;
pub mod outbound_network;
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
#[cfg(any(test, target_arch = "x86_64"))]
pub mod pointer_offer;
#[path = "presentation_nucleus/offers.rs"]
mod presentation_offers;
pub mod protection_domain;
#[cfg(any(test, target_arch = "x86_64"))]
pub mod usb_line_offer;
#[cfg(any(test, target_arch = "x86_64"))]
pub mod usb_line_session;
#[cfg(all(target_arch = "x86_64", feature = "virtio-net-proof"))]
pub mod virtio_net_proof;
#[cfg(all(target_arch = "x86_64", feature = "virtio-net-proof"))]
pub mod virtio_tcp;
#[cfg(all(target_arch = "x86_64", feature = "virtio-net-proof"))]
pub(crate) mod virtio_tcp_stream;
#[cfg(all(target_arch = "x86_64", feature = "virtio-net-proof"))]
pub mod virtio_tls;
#[cfg(all(
    target_arch = "x86_64",
    any(feature = "virtio-net-proof", feature = "hosted-tools")
))]
pub mod virtio_tls_fixture;
// Product entrances remain out of A0-A4 proof appliances. Non-x86_64 targets
// admit these modules only through distinct PROFILE-selected product features.
#[cfg(any(
    test,
    target_arch = "x86_64",
    feature = "ia32-product",
    feature = "aarch64-product",
    feature = "riscv64-product",
    feature = "loongarch64-product",
    feature = "aarch64-orange-pi-5"
))]
pub mod linear_presenter;
#[cfg(any(test, target_arch = "x86_64", feature = "hosted-tools"))]
pub mod presentation_nucleus;
#[cfg(any(
    test,
    target_arch = "x86_64",
    feature = "ia32-product",
    feature = "aarch64-product",
    feature = "riscv64-product",
    feature = "loongarch64-product",
    feature = "aarch64-orange-pi-5"
))]
mod presenter_control;
#[cfg(any(test, target_arch = "x86_64"))]
pub mod product_bases;
#[cfg(any(
    test,
    target_arch = "x86_64",
    feature = "ia32-product",
    feature = "aarch64-product",
    feature = "riscv64-product",
    feature = "loongarch64-product",
    feature = "aarch64-orange-pi-5"
))]
mod product_bindings;
#[cfg(all(target_arch = "x86_64", feature = "native-compositor"))]
pub mod product_front_door;
#[cfg(any(
    test,
    target_arch = "x86_64",
    feature = "ia32-product",
    feature = "aarch64-product",
    feature = "riscv64-product",
    feature = "loongarch64-product",
    feature = "aarch64-orange-pi-5"
))]
pub mod product_journey;
#[cfg(all(target_arch = "x86_64", feature = "native-compositor"))]
pub mod product_pointer;
#[cfg(any(test, target_arch = "x86_64"))]
pub mod product_usb_line;
#[cfg(target_arch = "x86_64")]
pub mod rescue_guest;
pub mod sign_format;
#[cfg(any(
    test,
    target_arch = "x86_64",
    feature = "ia32-product",
    feature = "aarch64-product",
    feature = "riscv64-product",
    feature = "loongarch64-product"
))]
pub mod spore_join;
pub mod spore_provision;
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
mod tour_comparison_kernel;
pub mod tour_comparison_plan;
mod tour_comparison_play;
mod tour_morse_kernel;
mod tour_morse_operations;
mod tour_morse_plan;
mod tour_morse_play;
pub mod tour_play;
#[cfg(any(test, target_arch = "x86_64"))]
pub mod tour_product;
#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
pub mod tour_shell;
mod tour_timer_kernel;
mod tour_timer_offer;
pub mod tour_timer_plan;
mod tour_timer_play;
mod tour_two_host;
mod tour_two_host_kernel;
pub mod tour_two_host_plan;
mod tour_two_host_play;
pub mod tour_workspace;
