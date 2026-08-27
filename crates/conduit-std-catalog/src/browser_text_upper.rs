use conduit_core::{ArtifactId, CapabilityOffer, ExecutionProfileId, ImplementationId};

use crate::text_upper_offer;

pub const BROWSER_TEXT_UPPER_PROFILE: &str = "browser/text-upper-kernel@1";
pub const BROWSER_TEXT_UPPER_ARTIFACT: &str = "conduit-browser-runtime/text-upper@1";
pub const BROWSER_TEXT_UPPER_IMPLEMENTATION: &str = "browser/text-upper@1";
pub const BROWSER_TEXT_UPPER_CAPABILITY: &str = "browser-text-upper-v1";

pub fn browser_text_upper_offer() -> CapabilityOffer {
    let mut offer = text_upper_offer();
    offer.capability_id = conduit_core::CapabilityId::from(BROWSER_TEXT_UPPER_CAPABILITY);
    offer.implementation.execution_profile_id =
        ExecutionProfileId::from(BROWSER_TEXT_UPPER_PROFILE);
    offer.implementation.implementation_id =
        ImplementationId::from(BROWSER_TEXT_UPPER_IMPLEMENTATION);
    offer.implementation.artifact_id = ArtifactId::from(BROWSER_TEXT_UPPER_ARTIFACT);
    offer
}
