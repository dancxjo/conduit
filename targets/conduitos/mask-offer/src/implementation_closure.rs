//! Package-owned finite native Back closure, distinct from live readiness.
use super::*;
use conduit_core::{BaseProviderEntry, CapabilityOffer};

pub const READ_ONLY_INTERACTION_PROFILE: &str = "conduitos/bounded-read-only-interaction@1";
pub const READ_ONLY_COLLECTION_INTERACTION_PROFILE: &str =
    "conduitos/bounded-read-only-collection-interaction@1";

/// Back inclusion facts, not resources, initialization, grants or live offers.
pub fn native_read_only_implementation_closure(
    profile: NativeMaskStorageProfile,
) -> Vec<CapabilityOffer> {
    let mut capabilities = renderer_host_for_profile(
        Adapter::Native,
        &"closure/host".into(),
        &"closure/boot".into(),
        1,
        None,
        profile,
    )
    .capabilities;
    narrow_read_only(&mut capabilities, profile);
    capabilities
}

/// Exact declared physical resources, no invented human-input pool. The narrowed
/// interaction profile can only close normally without producing an occurrence;
/// the execution wrapper must reject interaction delivery under this profile.
pub fn native_read_only_host_advertisement(
    host: &HostId,
    boot: &conduit_core::BootId,
    generation: u64,
    provider: &BaseProviderEntry,
    profile: NativeMaskStorageProfile,
) -> HostAdvertisement {
    let mut offer = renderer_host_for_profile(
        Adapter::Native,
        host,
        boot,
        generation,
        Some(provider),
        profile,
    );
    if !provider.capabilities.is_empty() {
        let base = offer
            .bases
            .iter_mut()
            .find(|base| base.provider_instance_id == provider.provider_instance_id);
        if let Some(base) = base {
            base.capability_ids = provider
                .capabilities
                .iter()
                .map(|capability| capability.capability_id.clone())
                .collect();
        }
    }
    offer.resources = provider.resources.clone();
    offer
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    narrow_read_only(&mut offer.capabilities, profile);
    offer
}

fn narrow_read_only(offers: &mut Vec<CapabilityOffer>, profile: NativeMaskStorageProfile) {
    // This entrance implements the ordinary native-graphical Source. Component
    // recipes require their own explicitly prepared renderer/Look closure.
    offers.retain(|offer| offer.kind_id.as_str() != NATIVE_COMPONENT_RENDERER_KIND);
    for offer in offers
        .iter_mut()
        .filter(|offer| offer.kind_id.as_str() == conduit_face::FACE_INTERACTION_KIND)
    {
        offer.implementation.execution_profile_id = match profile {
            NativeMaskStorageProfile::Standard => READ_ONLY_INTERACTION_PROFILE,
            NativeMaskStorageProfile::Collection => READ_ONLY_COLLECTION_INTERACTION_PROFILE,
        }
        .into();
        offer.resource_requirements.clear();
        // This Back returns normal exhaustion; it possesses no input device or
        // independent input authority. Renderer authority remains unchanged.
        offer.authority_requirements.clear();
    }
}

#[cfg(test)]
#[path = "implementation_closure/tests.rs"]
mod tests;
