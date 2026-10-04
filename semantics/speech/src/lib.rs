#![no_std]
//! Native segment contracts and fixed-storage speech Plot realizations.
// Uniform generated trees preserve authored grouping, eager comparisons, and
// one Option return convention rather than performing source-style rewrites.
#[allow(
    dead_code,
    unused_parens,
    clippy::double_parens,
    clippy::manual_range_contains,
    clippy::needless_question_mark
)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/voice.rs"));
}
mod pronounce;
mod render;
pub use generated::speech_realize as realize;
pub use generated::{
    EnglishDerivation, EnglishPhone, EnglishPhoneme, EnglishPosition, EnglishPronunciationOrigin,
    EnglishStress, RealizationInput, RealizationResult, SpeechCycleControlMode,
    SpeechEventVoiceControl, SpeechPhoneInput, TextSpeechSegment, VoiceBoundary, VoiceEvent,
    SOURCE_ID,
};
pub use pronounce::{
    pronounce, PronouncedText, TextRefusal, MAXIMUM_TEXT_BYTES, MAXIMUM_WORD_BYTES,
};
pub use render::{
    RenderRefusal, Renderer, MAXIMUM_BLOCK_FRAMES, MAXIMUM_EVENTS, MAXIMUM_UTTERANCE_FRAMES,
    SAMPLE_RATE_HZ,
};

#[cfg(any(feature = "semantic-bindings", feature = "kernel"))]
extern crate alloc;
#[cfg(feature = "semantic-bindings")]
pub mod admission;
#[cfg(feature = "semantic-bindings")]
pub mod boundary_admission;
#[cfg(feature = "semantic-bindings")]
pub mod control;
#[cfg(feature = "semantic-bindings")]
pub mod declared_realization;
#[cfg(feature = "semantic-bindings")]
pub mod duration;
#[cfg(feature = "semantic-bindings")]
pub mod intent_admission;
#[cfg(feature = "semantic-bindings")]
pub mod intent_inventory;
#[cfg(feature = "semantic-bindings")]
pub mod intent_prosody;
#[cfg(feature = "semantic-bindings")]
pub mod inventory_admission;
#[cfg(feature = "kernel")]
pub mod kernel;
#[cfg(feature = "semantic-bindings")]
pub mod profile_admission;
#[cfg(feature = "semantic-bindings")]
pub mod reference_admission;
#[cfg(feature = "semantic-bindings")]
pub mod text_admission;
#[cfg(feature = "semantic-bindings")]
pub mod timing;
#[cfg(feature = "semantic-bindings")]
pub mod utterance_timing;
/// Preparation/inspection bindings. The compact rendering Back does not carry
/// rich inventories or a heap. Bounds and local laws are checked here; resolving
/// artifact references remains an admission responsibility.
#[cfg(feature = "semantic-bindings")]
// Rich preparation values retain inline native variant payloads. Their finite
// collection bounds remain explicit; boxing is not a semantic requirement.
// This optional module is absent from the compact renderer's default profile.
#[allow(dead_code, clippy::large_enum_variant)]
pub mod semantic {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod control_parity;
#[cfg(test)]
mod differential;
#[cfg(test)]
mod duration_parity;
#[cfg(test)]
mod realization_parity;
#[cfg(test)]
mod timing_parity;

#[cfg(test)]
mod frame_parity;

#[cfg(test)]
mod pronunciation_parity;

#[cfg(test)]
mod trajectory_parity;

#[cfg(test)]
mod connection_parity;

#[cfg(test)]
mod prosody_parity;

#[cfg(test)]
mod onset_parity;

#[cfg(test)]
mod arithmetic_bounds;

#[cfg(test)]
mod neighbor_parity;

#[cfg(test)]
mod glottal_parity;

#[cfg(test)]
mod intensity_parity;

#[cfg(test)]
mod upper_source_parity;

#[cfg(test)]
mod frication_parity;

#[cfg(test)]
mod source_balance;

#[cfg(test)]
mod frication_transfer;
