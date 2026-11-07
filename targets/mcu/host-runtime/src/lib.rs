#![no_std]
//! Platform-neutral construction and bounded transport used by MCU Hosts.
//! These mechanisms do not issue authority, admit assignments, or run Plays.
pub mod membership;
pub mod seed;
pub mod transport;

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod membership_tests;
