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
    EnglishStress, RealizationInput, RealizationResult, TextSpeechSegment, VoiceBoundary,
    VoiceEvent, SOURCE_ID,
};
pub use pronounce::{
    pronounce, PronouncedText, TextRefusal, MAXIMUM_TEXT_BYTES, MAXIMUM_WORD_BYTES,
};
pub use render::{
    RenderRefusal, Renderer, MAXIMUM_BLOCK_FRAMES, MAXIMUM_EVENTS, MAXIMUM_UTTERANCE_FRAMES,
    SAMPLE_RATE_HZ,
};

#[cfg(feature = "semantic-bindings")]
extern crate alloc;
/// Preparation/inspection bindings. The compact rendering Back does not carry
/// rich inventories or a heap. Bounds and local laws are checked here; resolving
/// artifact references remains an admission responsibility.
#[cfg(feature = "semantic-bindings")]
#[allow(dead_code)]
pub mod semantic {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod differential;
#[cfg(test)]
mod realization_parity;

#[cfg(test)]
mod frame_parity;

#[cfg(test)]
mod pronunciation_parity;
