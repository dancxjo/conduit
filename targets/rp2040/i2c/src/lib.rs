//! RP2040 physical I2C custody for the shared class-neutral Host Call.
#![no_std]

#[cfg(feature = "rp2040")]
mod native;
#[cfg(any(feature = "rp2040", test))]
mod polling;
#[cfg(feature = "rp2040")]
pub use native::Rp2040I2c;
#[cfg(feature = "rp2040")]
mod hal;
#[cfg(feature = "rp2040")]
pub use hal::Error;
