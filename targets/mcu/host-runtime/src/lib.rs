#![no_std]
//! Platform-neutral construction and bounded transport used by MCU Hosts.
//! Construction and transport create no execution authority. The optional
//! execution path uses original Root grants, atomic admission and the ordinary
//! production kernel to run checked assignments.
pub mod image_memory;
pub mod ingress;
pub mod member_return;
pub mod membership;
pub mod poll_boundary;
pub mod root;
pub mod seed;
pub mod transport;

#[cfg(feature = "execution")]
pub mod execution;
#[cfg(feature = "execution")]
pub mod execution_authority;
#[cfg(feature = "execution")]
pub mod execution_factory;
#[cfg(feature = "execution")]
pub mod execution_mask;
#[cfg(feature = "execution")]
pub mod execution_software;
#[cfg(feature = "execution")]
pub mod execution_workset;
#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod membership_tests;

#[cfg(test)]
mod member_return_tests;
#[cfg(feature = "execution")]
pub mod native_images;

#[cfg(feature = "execution")]
pub mod native_dispatch;

#[cfg(feature = "execution")]
pub mod native_supervisor;

#[cfg(feature = "execution")]
pub mod execution_group;

#[cfg(feature = "execution")]
pub mod native_authority;

#[cfg(feature = "execution")]
pub mod native_runner;

#[cfg(feature = "execution")]
pub mod mask_session;

#[cfg(feature = "execution")]
pub mod kernel_sign;

#[cfg(feature = "execution")]
pub mod native_values;

#[cfg(feature = "execution")]
pub mod execution_inspection;

#[cfg(feature = "execution")]
pub mod continuation_native;

#[cfg(feature = "execution")]
pub mod recording_equipment;
#[cfg(feature = "execution")]
mod recording_residence;
#[cfg(feature = "execution")]
pub mod recording_service_equipment;

/// Descriptive installed cross-Boot metadata; never authority by decoding.
pub mod continuation_artifact;

#[cfg(feature = "execution")]
pub mod fore_bridge;
