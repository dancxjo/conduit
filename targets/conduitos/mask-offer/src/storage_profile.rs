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
    /// Transfer limits come from the actual finite implementation closure, never from
    /// an incoming chunk length. Requests/actions retain their separate bounds.
    pub fn owner_document_profile(host: &HostAdvertisement) -> Self {
        Self::from_native_implementations(host).unwrap_or(Self::Standard)
    }

    /// Select the exact initialized four-Back closure independently of the
    /// general Host profile. Every member must agree on finite execution limits.
    pub fn from_native_implementations(host: &HostAdvertisement) -> Result<Self, MaskOfferError> {
        let mut renderers = host
            .capabilities
            .iter()
            .filter(|offer| offer.capability_id.as_str() == "renderer-conduitos");
        let renderer = renderers.next().ok_or(MaskOfferError::InvalidPlan)?;
        if renderers.next().is_some() {
            return Err(MaskOfferError::InvalidPlan);
        }
        let profile = match renderer.implementation.execution_profile_id.as_str() {
            "conduitos/bounded-mask@1" => Self::Standard,
            "conduitos/bounded-collection-mask@1" => Self::Collection,
            _ => return Err(MaskOfferError::InvalidPlan),
        };
        profile.admit_native_implementation_profile(host, "renderer-conduitos")?;
        Ok(profile)
    }
    pub const fn owner_document_bytes(self) -> usize {
        match self {
            Self::Standard => 64 * 1024,
            Self::Collection => 384 * 1024,
        }
    }
    pub const fn owner_response_bytes(self) -> usize {
        match self {
            Self::Standard => conduit_face::MAX_OWNER_FACE_RESPONSE_BYTES,
            Self::Collection => self.owner_document_bytes() - 4 * 1024,
        }
    }
    pub const fn owner_document_chunks(self) -> usize {
        match self {
            Self::Standard => 16,
            Self::Collection => 64,
        }
    }
    pub(super) const fn renderer_queue_bytes(self) -> u32 {
        match self {
            Self::Standard => MAX_RENDERER_VALUE_BYTES,
            Self::Collection => self.bytes() * 2,
        }
    }

    /// Explicit native implementation closure admission. A general Host's
    /// profile label is independent of this selected finite Mask profile.
    pub(super) fn admit_native_implementation_profile(
        self,
        host: &HostAdvertisement,
        renderer: &str,
    ) -> Result<(), MaskOfferError> {
        if host.protocol_version != PROTOCOL_VERSION || host.offer_generation.0 == 0 {
            return Err(MaskOfferError::InvalidPlan);
        }
        self.admit_native_backs(host, renderer)
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
        self.admit_native_backs(host, renderer)
    }
    fn admit_native_backs(
        self,
        host: &HostAdvertisement,
        renderer: &str,
    ) -> Result<(), MaskOfferError> {
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
            let read_only = id == "renderer-conduitos-input"
                && offer.implementation.execution_profile_id.as_str()
                    == match self {
                        Self::Standard => READ_ONLY_INTERACTION_PROFILE,
                        Self::Collection => READ_ONLY_COLLECTION_INTERACTION_PROFILE,
                    }
                && offer.resource_requirements.is_empty()
                && offer.authority_requirements.is_empty();
            if matching.next().is_some()
                || (!read_only
                    && offer.implementation.execution_profile_id.as_str()
                        != if id == "renderer-conduitos-input" {
                            match self {
                                Self::Standard => "conduitos/bounded-interaction@1",
                                Self::Collection => "conduitos/bounded-collection-interaction@1",
                            }
                        } else {
                            self.execution_profile()
                        })
                || offer.limits.max_queue_items == 0
                || offer.limits.max_active_instances == 0
                || offer.limits.max_queue_bytes
                    < if id == "renderer-conduitos-input" && self == Self::Standard {
                        conduit_face::MAX_FACE_INTERACTION_BYTES as u32
                    } else {
                        self.bytes()
                    }
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
    layout_configured::prepare_offered_component_document(
        host,
        (
            conduit_face::TODO_COMPONENT_MASK_SOURCE,
            conduit_face::TODO_LAYOUT_MASK_SOURCE,
        ),
        selection.todo_root(recipe),
        NativeMaskStorageProfile::Collection,
    )
}

pub fn prepare_collection_creche_stage_from_offer(
    host: &HostAdvertisement,
    selection: ComponentVisualSelection,
    recipe: CrecheComponentRecipe,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    layout_configured::prepare_offered_component_document(
        host,
        (
            conduit_face::CRECHE_COMPONENT_MASK_SOURCE,
            conduit_face::CRECHE_LAYOUT_MASK_SOURCE,
        ),
        selection.creche_root(recipe),
        NativeMaskStorageProfile::Collection,
    )
}

/// Complete reviewed visual roots with matching explicitly admitted Collection storage.
pub fn prepare_collection_creche_visual_stage_from_offer(
    host: &HostAdvertisement,
    selection: ComponentVisualSelection,
    recipe: CrecheComponentRecipe,
    typography: conduit_face::VisualTypography,
    navy: bool,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    let root = if navy {
        selection.creche_navy_root(recipe, typography)
    } else {
        selection.creche_root_with_typography(recipe, typography)
    };
    layout_configured::prepare_offered_component_document(
        host,
        (
            conduit_face::CRECHE_COMPONENT_MASK_SOURCE,
            conduit_face::CRECHE_LAYOUT_MASK_SOURCE,
        ),
        root,
        NativeMaskStorageProfile::Collection,
    )
}

#[cfg(test)]
mod tests;

/// Plan against the exact already-observed native advertisement. Existing
/// standard-only entrances never call this and never upgrade themselves.
/// Membership, carrier presence and authority must be established by the caller.
pub fn prepare_advertised_native_stage_from_offer(
    host: &HostAdvertisement,
    selection: Option<(ComponentVisualSelection, TodoComponentRecipe)>,
) -> Result<MaskStage, MaskOfferError> {
    if selection.is_none() {
        return prepare_native_stage_with_profile(
            host,
            NativeMaskStorageProfile::from_native_implementations(host)?,
        );
    }
    let profile = match host.profile.as_str() {
        "conduitos/mask-host@1" => NativeMaskStorageProfile::Standard,
        "conduitos/collection-mask-host@1" => NativeMaskStorageProfile::Collection,
        _ => return Err(MaskOfferError::InvalidPlan),
    };
    match (profile, selection) {
        (NativeMaskStorageProfile::Standard, None) => {
            prepare_stage_from_offer(Adapter::Native, host)
        }
        (NativeMaskStorageProfile::Collection, None) => prepare_collection_stage_from_offer(host),
        (NativeMaskStorageProfile::Standard, Some((look, recipe))) => {
            prepare_selected_component_stage_from_offer(host, look, recipe).map(|(stage, _)| stage)
        }
        (NativeMaskStorageProfile::Collection, Some((look, recipe))) => {
            prepare_collection_component_stage_from_offer(host, look, recipe)
                .map(|(stage, _)| stage)
        }
    }
}
