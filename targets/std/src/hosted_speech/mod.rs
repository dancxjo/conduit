//! Bounded hosted speech-synthesis providers.

mod piper;

pub use piper::{
    PiperDiscovery, PiperFailure, PiperLimits, PiperSpeechAdapter, PiperSynthesisReceipt,
};
