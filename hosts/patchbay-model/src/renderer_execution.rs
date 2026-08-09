//! Exact planned lifecycle for a concrete renderer adapter.

use conduit_core::{
    bind_active_play, kind_id, resource_offer, resource_requirement, ActivePlayId, ArtifactId,
    BootId, CapabilityId, CapabilityLimits, ClueId, ExecutionProfileId, HostAdvertisement, HostId,
    HostOperationContractId, HostOperationRequirement, HostProfileId, ImplementationId,
    OfferGeneration, PlacementId, Plan, PROTOCOL_VERSION,
};
use conduit_form::{parse, ProfileCatalog};
use conduit_planner::{default_placements, plan};
use conduit_presentation::{
    renderer_kind_definition, renderer_offer, Manifestation, ManifestationError,
    ManifestationFailure, ManifestationLifecycle, Presentation, RendererRealizationOffer,
    MAX_RENDERER_VALUE_BYTES,
};

use crate::{RendererSelfInspection, RendererSelfInspectionError};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RendererAdapterKind {
    NativeWayland,
    HtmlDomSvg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererAdapterIdentity {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub target_subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererExecution {
    pub presentation: Presentation,
    pub plan: Plan,
    pub active_play_id: ActivePlayId,
    pub placement_id: PlacementId,
    pub manifestation: Manifestation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererExecutionError {
    InvalidRendererForm,
    Planning,
    MissingPlacement,
    Manifestation(ManifestationError),
    Inspection(RendererSelfInspectionError),
}

impl core::fmt::Display for RendererExecutionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "renderer execution failed: {self:?}")
    }
}

impl std::error::Error for RendererExecutionError {}

impl RendererExecution {
    pub fn prepare(
        presentation: Presentation,
        adapter: RendererAdapterKind,
        identity: RendererAdapterIdentity,
        clue_id: ClueId,
    ) -> Result<Self, RendererExecutionError> {
        presentation.validate().map_err(|_| {
            RendererExecutionError::Manifestation(ManifestationError::InvalidPresentation)
        })?;
        let form = renderer_form()?;
        let advertisement = renderer_host(adapter, &identity);
        let placements = default_placements(&form, core::slice::from_ref(&advertisement))
            .map_err(|_| RendererExecutionError::Planning)?;
        let plan = plan(&form, &[advertisement], &placements, &[])
            .map_err(|_| RendererExecutionError::Planning)?;
        let fragment = plan
            .fragments
            .first()
            .ok_or(RendererExecutionError::MissingPlacement)?;
        let placement_id = fragment
            .placements
            .first()
            .map(|placement| placement.placement_id.clone())
            .ok_or(RendererExecutionError::MissingPlacement)?;
        let active_play_id =
            bind_active_play(&plan.plan_id, &identity.host_id, &identity.boot_id, 0).active_play_id;
        let manifestation = Manifestation::prepared(
            &presentation,
            &plan,
            active_play_id.clone(),
            placement_id.clone(),
            identity.target_subject,
            clue_id,
        )
        .map_err(RendererExecutionError::Manifestation)?;
        Ok(Self {
            presentation,
            plan,
            active_play_id,
            placement_id,
            manifestation,
        })
    }

    pub fn mark_available(&mut self, clue_id: ClueId) -> Result<(), RendererExecutionError> {
        if self.manifestation.lifecycle == ManifestationLifecycle::Available {
            return Ok(());
        }
        self.manifestation = self
            .manifestation
            .transition(ManifestationLifecycle::Available, clue_id)
            .map_err(RendererExecutionError::Manifestation)?;
        Ok(())
    }

    pub fn mark_failed(
        &mut self,
        failure: ManifestationFailure,
        clue_id: ClueId,
    ) -> Result<(), RendererExecutionError> {
        self.manifestation = self
            .manifestation
            .fail(failure, clue_id)
            .map_err(RendererExecutionError::Manifestation)?;
        Ok(())
    }

    pub fn mark_closed(&mut self, clue_id: ClueId) -> Result<(), RendererExecutionError> {
        self.manifestation = self
            .manifestation
            .transition(ManifestationLifecycle::Closed, clue_id)
            .map_err(RendererExecutionError::Manifestation)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), RendererExecutionError> {
        self.manifestation
            .validate_against(&self.presentation, &self.plan)
            .map_err(RendererExecutionError::Manifestation)?;
        if self.manifestation.active_play_id != self.active_play_id
            || self.manifestation.placement_id != self.placement_id
        {
            return Err(RendererExecutionError::Manifestation(
                ManifestationError::StaleIdentity,
            ));
        }
        Ok(())
    }

    pub fn self_inspection(&self) -> Result<RendererSelfInspection, RendererExecutionError> {
        self.validate()?;
        RendererSelfInspection::new(
            &self.presentation,
            self.plan.clone(),
            self.manifestation.clone(),
        )
        .map_err(RendererExecutionError::Inspection)
    }
}

fn renderer_form() -> Result<conduit_form::CheckedForm, RendererExecutionError> {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(renderer_kind_definition())
        .map_err(|_| RendererExecutionError::InvalidRendererForm)?;
    parse(
        "form 0\n\npatchbay-show {\n    renderer: presentation/renderer\n}\n",
        &catalog,
    )
    .map_err(|_| RendererExecutionError::InvalidRendererForm)
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
            host_operation: HostOperationRequirement {
                contract_id: HostOperationContractId::from("conduit.host/present@1"),
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
