//! Bounded hosted speech-synthesis providers.

mod generated_manifestation;
mod piper;
mod session;

pub use generated_manifestation::{GeneratedSpeechReceipt, GeneratedSpeechRefusal};
pub use piper::{
    PiperDiscovery, PiperFailure, PiperLimits, PiperSpeechAdapter, PiperSynthesisReceipt,
};
pub use session::PiperSynthesisStep;
