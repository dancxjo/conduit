//! Exact planned lifecycle for a concrete renderer adapter.

use conduit_core::{
    kind_id, resource_offer, resource_requirement, ArtifactId, BootId, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostCallContractId,
    HostCallRequirement, HostId, HostProfileId, ImplementationId, OfferGeneration, SignId,
    PROTOCOL_VERSION,
};
use conduit_form::{parse, ProfileCatalog};
use conduit_planner::{default_placements, plan};
use conduit_presentation::{
    renderer_kind_projection, renderer_offer, Presentation, RendererExecution,
    RendererExecutionError, RendererRealizationOffer, MAX_RENDERER_VALUE_BYTES,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RendererAdapterKind {
    NativeWayland,
    HtmlDomSvg,
    TestSpeech,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererAdapterIdentity {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub target_subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererPreparationError {
    InvalidRendererForm,
    Planning,
    Execution(RendererExecutionError),
}

impl core::fmt::Display for RendererPreparationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "renderer preparation failed: {self:?}")
    }
}

impl std::error::Error for RendererPreparationError {}

pub fn prepare_renderer_execution(
    presentation: Presentation,
    adapter: RendererAdapterKind,
    identity: RendererAdapterIdentity,
    sign_id: SignId,
) -> Result<RendererExecution, RendererPreparationError> {
    prepare_renderer_execution_with_offer_generation(presentation, adapter, identity, 1, sign_id)
}

pub fn prepare_renderer_execution_with_offer_generation(
    presentation: Presentation,
    adapter: RendererAdapterKind,
    identity: RendererAdapterIdentity,
    offer_generation: u64,
    sign_id: SignId,
) -> Result<RendererExecution, RendererPreparationError> {
    let form = renderer_form()?;
    let mut advertisement = renderer_host(adapter, &identity);
    advertisement.offer_generation = OfferGeneration(offer_generation);
    let placements = default_placements(&form, core::slice::from_ref(&advertisement))
        .map_err(|_| RendererPreparationError::Planning)?;
    let plan = plan(&form, &[advertisement], &placements, &[])
        .map_err(|_| RendererPreparationError::Planning)?;
    RendererExecution::prepare_planned(presentation, plan, identity.target_subject, sign_id)
        .map_err(RendererPreparationError::Execution)
}

fn renderer_form() -> Result<conduit_form::CheckedForm, RendererPreparationError> {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(renderer_kind_projection())
        .map_err(|_| RendererPreparationError::InvalidRendererForm)?;
    parse(
        "form patchbay-show {\n    renderer: presentation/renderer\n}\n",
        &catalog,
    )
    .map_err(|_| RendererPreparationError::InvalidRendererForm)
}

pub(crate) fn renderer_host(
    adapter: RendererAdapterKind,
    identity: &RendererAdapterIdentity,
) -> HostAdvertisement {
    let (capability, implementation, artifact, target_kind, resource_class) = match adapter {
        RendererAdapterKind::NativeWayland => (
            "renderer-wayland",
            "presentation/renderer-wayland@1",
            "patchbay-native/wayland@1",
            "presentation/base/wayland-surface@1",
            "conduit.resource/wayland-surface@1",
        ),
        RendererAdapterKind::HtmlDomSvg => (
            "renderer-dom-svg",
            "presentation/renderer-dom-svg@1",
            "patchbay-html/dom-svg@1",
            "presentation/base/dom-svg@1",
            "conduit.resource/browser-document@1",
        ),
        RendererAdapterKind::TestSpeech => (
            "renderer-test-speech",
            "presentation/renderer-test-speech@1",
            "patchbay/test-speech@1",
            "presentation/base/test-speech@1",
            "conduit.resource/test-speech-sink@1",
        ),
    };
    let limits = CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
    };
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("presentation/host@1"),
        bases: vec![],
        resources: vec![resource_offer(
            &format!("{}/presentation", identity.host_id.as_str()),
            resource_class,
            1,
        )],
        capabilities: vec![renderer_offer(RendererRealizationOffer {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from("presentation/renderer-hosted@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_call: HostCallRequirement {
                contract_id: HostCallContractId::from("conduit.host/present@1"),
                target_kind: Some(kind_id(target_kind)),
                maximum_in_flight: 1,
                maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
                maximum_output_bytes: MAX_RENDERER_VALUE_BYTES,
            },
            resource_requirement: resource_requirement(resource_class, 1),
            limits,
        })],
        planner_capabilities: vec![],
    }
}
