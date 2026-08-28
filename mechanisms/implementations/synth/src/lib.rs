#![no_std]

//! Fixed-storage deterministic reference implementation of `music/synth`.
//!
//! This crate owns DSP and voice state only. Scheduling, Cord pressure, value
//! retention, Plans, Plays, and Signs remain owned by the production kernel.
//!
//! The `fixed-q16@1` profile uses integer phase accumulators and integer DSP,
//! rounds envelope durations upward to whole 48 kHz frames, saturates only at
//! the final signed-16 mixer boundary, and has no floating-point, NaN, denormal,
//! or implementation-dependent SIMD behavior. Invalid controls and unstable
//! filter/profile bounds are refused before state mutation. Exact-reference
//! conformance is byte equality; alternate declared implementations need their
//! own tolerance profile.

mod engine;
mod envelope;
mod filter;
mod oscillator;
mod profile;
mod voice;

pub use engine::*;
pub use envelope::*;
pub use filter::*;
pub use oscillator::*;
pub use profile::*;
pub use voice::*;

pub const REFERENCE_SYNTH_PROFILE_ID: &str = "conduit.reference/music-synth-fixed-q16@1";
pub const REFERENCE_SYNTH_IMPLEMENTATION_ID: &str = "std/kernel-music-synth-fixed-q16@1";
pub const REFERENCE_SYNTH_ARTIFACT_ID: &str = "conduit-std-host/music-synth-fixed-q16@1";
pub const REFERENCE_SAMPLE_RATE_HZ: u32 = 48_000;
pub const REFERENCE_MAXIMUM_VOICES: usize = 16;
pub const REFERENCE_MAXIMUM_BLOCK_FRAMES: u16 = 256;
