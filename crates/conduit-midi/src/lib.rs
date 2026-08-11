#![no_std]

//! Bounded MIDI 1.0 protocol realization for Conduit's portable music Info.
//!
//! MIDI channel, key, controller, status, and byte-stream facts remain in
//! this crate. They never become the definition of portable musical meaning.

mod adapter;
mod message;
mod parser;

pub use adapter::{
    midi_velocity_to_portable, MidiAdapterError, MidiInputAdapter, MidiOutputAdapter, MidiProfile,
    PortableMidiEvent, MIDI_PITCH_BEND_RANGE_MICROCENTS,
};
pub use message::MidiMessage;
pub use parser::{MidiParseError, MidiParser, ParsedMidi};
