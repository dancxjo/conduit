#![no_std]
//! Finite retained composition. No allocation, device access or semantic authority.
mod damage;
mod frame;
mod lifecycle;
mod scanline;
mod state;
mod upload;
pub mod wire;
use damage::Damage;
pub use frame::{CompletedFrame, FramePlan, FrameRect, Span};
pub use lifecycle::Route;
pub use state::{Compositor, Error, Handle, MAX_CHUNK, MAX_PIXELS, MAX_SURFACES, Rect};
pub(crate) use state::{Surface, Upload};
#[cfg(test)]
extern crate std;
#[cfg(test)]
mod frame_plan_tests;
#[cfg(test)]
mod tests;
