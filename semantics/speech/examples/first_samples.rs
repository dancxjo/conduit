//! Internal artifact writer; xtask will own the supported proof entrance.
use conduit_speech::{
    EnglishPhoneme as P, EnglishPosition as W, EnglishStress as S, RealizationInput, Renderer,
    VoiceBoundary, VoiceEvent, SAMPLE_RATE_HZ,
};
use std::{fs, io::Write};
#[cfg(feature = "semantic-bindings")]
#[path = "first_samples/control.rs"]
mod control_samples;
#[cfg(feature = "semantic-bindings")]
#[path = "first_samples/duration.rs"]
mod duration_samples;
