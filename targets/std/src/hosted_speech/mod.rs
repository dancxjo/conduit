//! Bounded hosted speech-synthesis providers.

mod piper;
mod session;

pub use piper::{
    PiperDiscovery, PiperFailure, PiperLimits, PiperSpeechAdapter, PiperSynthesisReceipt,
};
pub use session::PiperSynthesisStep;

pub(crate) fn process_resource_offer() -> conduit_core::ResourceOffer {
    conduit_core::resource_offer(
        "std/piper-process",
        conduit_std_offers::PIPER_PROCESS_RESOURCE_CLASS,
        1,
    )
}
