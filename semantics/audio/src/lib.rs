#![no_std]

//! Portable bounded audio and musical value contracts.
//!
//! Host callbacks, devices, MIDI/OPL protocols, DSP implementations,
//! scheduling, Plans, Plays, and Signs remain with their owning layers.

extern crate alloc;

mod audio_info;
mod audio_render_demand;
mod sampled_signal_mapping;
mod sound_info;

pub use audio_info::*;
pub use audio_render_demand::*;
pub use sampled_signal_mapping::*;
pub use sound_info::*;
