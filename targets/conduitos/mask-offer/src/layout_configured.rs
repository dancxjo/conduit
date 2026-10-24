//! Explicit selected-Back preparation for the complete-Look layout contract.
//! Deployment explicitly offers initialized layout support. Callers must retain
//! an observed matching offer; availability is not inferred from viewport size.
use super::*;
use conduit_core::*;
use conduit_face::{ConfiguredMaskConfiguration, layout_component_mask_kind};

pub const NATIVE_LAYOUT_COMPONENT_RENDERER_KIND: &str =
    "face/native-layout-component-resource-mask";
pub const NATIVE_LAYOUT_COMPONENT_CAPABILITY: &str = "renderer-conduitos-layout-component";

const BACK_SOURCE: &str = r#"plot native-layout-component-back (
 label: Text
 recipe: ComponentRecipeInfo
 look: LookInfo
 layout: ComponentLayoutInfo
 >> face: Face
 interaction: FaceInteraction...| >>
 show: Show >>
) {
 spread: face/tee("Shared Face")
 surface: face/show-resource-source("Output resource")
 render: face/native-layout-component-resource-mask(label = label, recipe = recipe, look = look, layout = layout)
 human: face/interaction("User actions")
 face >> spread.source
 spread.face >> render.face
 surface.show-resource >> render.show-resource
 spread.face >> human.face
 render.show >> human.show
 render.show >> show
 human.interaction >> interaction
}
"#;

pub fn layout_resource_renderer_kind(profile: NativeMaskStorageProfile) -> Kind {
    let component = layout_component_mask_kind(profile.bytes());
    let mut kind = resource_renderer_kind();
    kind.kind_id = kind_id(NATIVE_LAYOUT_COMPONENT_RENDERER_KIND);
    kind.kind_contract_revision = "conduit.face/native-layout-component-resource-mask@2".into();
    kind.startup_parameters = component.startup_parameters;
    kind.configuration = component.configuration;
    kind
}

pub fn layout_resource_renderer_offer(profile: NativeMaskStorageProfile) -> CapabilityOffer {
    let mut offer = BackOfferBuilder::new(
        layout_resource_renderer_kind(profile),
        Back {
            capability_id: NATIVE_LAYOUT_COMPONENT_CAPABILITY.into(),
            execution_profile_id: profile.execution_profile().into(),
            implementation_id: "face/renderer-conduitos-layout-component@1".into(),
            artifact_id: "conduitos/native-compositor@1".into(),
            host_calls: vec![HostCallRequirement {
                contract_id: "conduit.host/present@1".into(),
                target_kind: Some(kind_id("face/base/conduitos-surface@1")),
                maximum_in_flight: 1,
                maximum_input_bytes: profile.bytes(),
                maximum_output_bytes: profile.bytes(),
            }],
            resource_requirements: vec![],
            authority_requirements: vec![present_authority_requirement(kind_id(
                "face/base/conduitos-surface@1",
            ))],
        },
    )
    .build();
    if profile == NativeMaskStorageProfile::Collection {
        offer.limits.max_queue_bytes = profile.renderer_queue_bytes();
    }
    offer
}

/// Caller supplies an actually observed matching offer. This does not upgrade
/// an old renderer or infer support from available memory or viewport width.
pub fn prepare_layout_component_document(
    host: &HostAdvertisement,
    source: &str,
    root: &str,
    profile: NativeMaskStorageProfile,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    configured::prepare_renderer_document(
        host,
        source,
        root,
        profile,
        configured::RendererContract {
            component: layout_component_mask_kind(profile.bytes()),
            renderer: layout_resource_renderer_kind(profile),
            kind_name: NATIVE_LAYOUT_COMPONENT_RENDERER_KIND,
            back_source: BACK_SOURCE,
            back_root: "native-layout-component-back",
            capability: NATIVE_LAYOUT_COMPONENT_CAPABILITY,
        },
    )
}

/// Deployment entrance for a native consumer implementing checked layouts.
/// Storage and surface possession remain the caller's explicit preparation.
/// Older native advertisements retain their version-two contract.
pub fn native_layout_host_advertisement(
    host: &HostId,
    boot: &BootId,
    generation: u64,
    surface: &BaseProviderEntry,
    profile: NativeMaskStorageProfile,
) -> HostAdvertisement {
    let mut offer = renderer_host_for_profile(
        Adapter::Native,
        host,
        boot,
        generation,
        Some(surface),
        profile,
    );
    // This deployment realizes the new contract instead of advertising two
    // configured renderers and duplicating their startup schemas in every join.
    offer
        .capabilities
        .retain(|back| back.capability_id.as_str() != "renderer-conduitos-component");
    offer
        .capabilities
        .push(layout_resource_renderer_offer(profile));
    offer
}

/// Select from the exact observed offer. A malformed advertised layout Back
/// refuses in checked planning; only its absence retains the older contract.
pub(super) fn prepare_offered_component_document(
    host: &HostAdvertisement,
    sources: (&str, &str),
    root: &str,
    profile: NativeMaskStorageProfile,
) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
    if host
        .capabilities
        .iter()
        .any(|offer| offer.capability_id.as_str() == NATIVE_LAYOUT_COMPONENT_CAPABILITY)
    {
        prepare_layout_component_document(host, sources.1, root, profile)
    } else {
        configured::prepare_component_document(host, sources.0, root, profile)
    }
}

#[cfg(test)]
mod tests;
