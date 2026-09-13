//! Bounded hosted speech-synthesis providers.

mod piper;
mod session;

pub use piper::{
    PiperDiscovery, PiperFailure, PiperLimits, PiperSpeechAdapter, PiperSynthesisReceipt,
};
pub use session::PiperSynthesisStep;
