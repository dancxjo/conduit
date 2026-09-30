#![no_std]

//! Portable bounded audio and musical value contracts.
//!
//! Host callbacks, devices, MIDI/OPL protocols, DSP implementations,
//! scheduling, Plans, Plays, and Signs remain with their owning layers.

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    AudioToneTerminal, AudioToneTerminalRepresentation, CancellationDisposition, Gate,
    ModulationDestination, MusicalControl, MusicalControlModulation, MusicalControlPitchBend,
    MusicalControlSustain, PcmChannelLayout, PcmSampleRepresentation, PressureDisposition,
    SoundSeam, SoundStreamState, SoundTerminalBehavior,
};

mod audio_info;
mod audio_render_demand;
mod pcm_clip;
mod sampled_signal_mapping;
mod sound_info;
mod tone_terminal;

pub use audio_info::*;
pub use audio_render_demand::*;
pub use pcm_clip::*;
pub use sampled_signal_mapping::*;
pub use sound_info::*;
pub use tone_terminal::*;
