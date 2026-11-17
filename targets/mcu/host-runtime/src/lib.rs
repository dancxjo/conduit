#![no_std]
//! Shared native membership and bounded signed-input custody.
//! These mechanisms create no execution authority or alternate Root/kernel.
pub mod ingress;
pub mod member_return;
#[cfg(test)]
mod member_return_tests;
pub mod membership;
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
