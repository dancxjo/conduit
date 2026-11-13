#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

#[cfg(not(all(target_arch = "aarch64", feature = "aarch64-rpi-3b-plus")))]
compile_error!("the Raspberry Pi 3B+ product requires its exact AArch64 board feature");
include!("raspberry_pi_product/mod.rs");
