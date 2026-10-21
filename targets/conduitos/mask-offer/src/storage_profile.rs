//! Explicit storage admission. The default never discovers or upgrades itself.
use super::*;
use conduit_core::BaseProviderEntry;
use conduit_face::{
    ComponentVisualSelection, ConfiguredMaskConfiguration, CrecheComponentRecipe,
    TodoComponentRecipe,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeMaskStorageProfile {
    Standard,
    Collection,
}
impl NativeMaskStorageProfile {
    pub const fn bytes(self) -> u32 {
        match self {
            Self::Standard => MASK_BYTES as u32,
            Self::Collection => 256 * 1024,
        }
    }
    pub const fn host_profile(self) -> &'static str {
        match self {
            Self::Standard => "conduitos/mask-host@1",
            Self::Collection => "conduitos/collection-mask-host@1",
        }
    }
    pub const fn execution_profile(self) -> &'static str {
        match self {
            Self::Standard => "conduitos/bounded-mask@1",
            Self::Collection => "conduitos/bounded-collection-mask@1",
        }
    }
    pub(super) const fn renderer_queue_bytes(self) -> u32 {
        match self {
            Self::Standard => MAX_RENDERER_VALUE_BYTES,
            Self::Collection => self.bytes() * 2,
        }
    }

    pub(super) fn admit_host(
        self,
        host: &HostAdvertisement,
        renderer: &str,
    ) -> Result<(), MaskOfferError> {
        if host.protocol_version != PROTOCOL_VERSION
            || host.profile.as_str() != self.host_profile()
            || host.offer_generation.0 == 0
        {
            return Err(MaskOfferError::InvalidPlan);
        }
        if self == Self::Standard {
            return Ok(());
        }
        // A profile label alone cannot upgrade a smaller initialized offer.
        for id in [
            renderer,
            "renderer-conduitos-tee",
            "renderer-conduitos-resource-source",
            "renderer-conduitos-input",
        ] {
            let mut matching = host
                .capabilities
                .iter()
                .filter(|offer| offer.capability_id.as_str() == id);
            let offer = matching.next().ok_or(MaskOfferError::InvalidPlan)?;
            if matching.next().is_some()
                || offer.implementation.execution_profile_id.as_str()
                    != if id == "renderer-conduitos-input" {
                        "conduitos/bounded-collection-interaction@1"
                    } else {
                        self.execution_profile()
                    }
                || offer.limits.max_queue_items == 0
                || offer.limits.max_queue_bytes < self.bytes()
            {
                return Err(MaskOfferError::InvalidPlan);
            }
            if id == renderer {
                let mut calls = offer
                    .host_calls
                    .iter()
                    .filter(|call| call.contract_id.as_str() == "conduit.host/present@1");
                let call = calls.next().ok_or(MaskOfferError::InvalidPlan)?;
                if calls.next().is_some()
                    || call.target_kind.as_ref().map(|kind| kind.as_str())
                        != Some("face/base/conduitos-surface@1")
                    || call.maximum_in_flight != 1
                    || call.maximum_input_bytes != self.bytes()
                    || call.maximum_output_bytes != self.bytes()
                {
                    return Err(MaskOfferError::InvalidPlan);
                }
            }
        }
        Ok(())
    }
}

/// Use only after the Host initializes matching collection storage. This does
/// not initialize a renderer, admit membership, or enlarge a transport frame.
pub fn collection_native_host_advertisement(
    host: &HostId,
    boot: &conduit_core::BootId,
    generation: u64,
    surface: &BaseProviderEntry,
) -> HostAdvertisement {
    renderer_host_for_profile(
        Adapter::Native,
        host,
        boot,
        generation,
        Some(surface),
        NativeMaskStorageProfile::Collection,
    )
}

pub fn prepare_collection_stage_from_offer(
    host: &HostAdvertisement,
) -> Result<MaskStage, MaskOfferError> {
    prepare_stage_for_profile(Adapter::Native, host, NativeMaskStorageProfile::Collection)
}

pub fn prepare_collection_component_stage_from_offer(
    host: &HostAdvertisement,
    selection: ComponentVisualSelection,
    recipe: TodoComponentRecipe,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    configured::prepare_component_document(
        host,
        conduit_face::TODO_COMPONENT_MASK_SOURCE,
        selection.todo_root(recipe),
        NativeMaskStorageProfile::Collection,
    )
}

pub fn prepare_collection_creche_stage_from_offer(
    host: &HostAdvertisement,
    selection: ComponentVisualSelection,
    recipe: CrecheComponentRecipe,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    configured::prepare_component_document(
        host,
        conduit_face::CRECHE_COMPONENT_MASK_SOURCE,
        selection.creche_root(recipe),
        NativeMaskStorageProfile::Collection,
    )
}

#[cfg(test)]
mod tests;
