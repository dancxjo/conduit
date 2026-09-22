//! Bounded hosted speech-synthesis providers.

mod generated_manifestation;
mod piper;
mod session;

pub use generated_manifestation::{GeneratedSpeechReceipt, GeneratedSpeechRefusal};
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

#[cfg(feature = "local-model-proof")]
pub(crate) fn process_realization_advertisement(
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
    offer_generation: conduit_core::OfferGeneration,
) -> conduit_core::RealizationAdvertisement {
    conduit_core::RealizationAdvertisement {
        host_id,
        boot_id,
        offer_generation,
        capability_id: conduit_std_offers::piper_speech_offer().capability_id,
        characteristics: Vec::new(),
    }
}

#[cfg(feature = "local-model-proof")]
pub(crate) fn process_resource_observation(
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
    offer_generation: conduit_core::OfferGeneration,
    sign_id: conduit_core::SignId,
) -> conduit_core::ResourceObservation {
    conduit_core::ResourceObservation {
        host_id,
        boot_id,
        offer_generation,
        pool_id: conduit_core::ResourcePoolId::from("std/piper-process"),
        class_id: conduit_core::ResourceClassId::from(
            conduit_std_offers::PIPER_PROCESS_RESOURCE_CLASS,
        ),
        health: conduit_core::ResourceHealth::Ready,
        unreserved_units: 1,
        utilized_units: 0,
        sign_id,
    }
}
