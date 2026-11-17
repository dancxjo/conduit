#![no_std]
//! Shared native membership and bounded signed-input custody.
//! These mechanisms create no execution authority or alternate Root/kernel.
pub mod ingress;
#[cfg(feature = "member-return")]
pub mod member_return;
#[cfg(all(test, feature = "member-return"))]
mod member_return_tests;
pub mod membership;
#[cfg(feature = "return-receiver")]
pub mod return_receiver;
pub mod transport;
#[cfg(test)]
#[macro_use]
extern crate std;
#[cfg(test)]
mod membership_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod ingress_tests;
