//! Exact optional SSD1306 Presenter realization over ordinary Presentation.

use conduit_core::{
    bind_active_play, resource_offer, resource_requirement, ArtifactId, BootId, CapabilityId,
    CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, HostProfileId, ImplementationId, OfferGeneration, Plan, SignId,
    PROTOCOL_VERSION,
};
use conduit_form::{parse, ProfileCatalog};
use conduit_planner::{default_placements, plan};
use conduit_presentation::{
    renderer_kind_definition, renderer_offer, Manifestation, ManifestationFailure,
    ManifestationLifecycle, Presentation, PresentationRole, RendererRealizationOffer,
    MAX_RENDERER_VALUE_BYTES,
};
use conduit_ssd1306::{Ssd1306Failure, Ssd1306I2cProvider, Ssd1306Session};

use crate::{project_ssd1306_frame, Ssd1306Frame, Ssd1306ProjectionError};

pub const SSD1306_PRESENTER_IMPLEMENTATION: &str = "netherwick/presenter-ssd1306-128x32@1";
pub const SSD1306_PRESENTER_CAPABILITY: &str = "netherwick/presenter-ssd1306@1";
pub const SSD1306_PRESENTER_PROFILE: &str = "netherwick/ssd1306-product@1";
pub const SSD1306_PRESENTER_ARTIFACT: &str = "conduit-netherwick/ssd1306-presenter@1";
pub const SSD1306_PRESENT_OPERATION: &str = "conduit.host/present-ssd1306@1";
pub const SSD1306_I2C_RESOURCE: &str = "netherwick.resource/ssd1306-i2c-base@1";
pub const SSD1306_ATTACHMENT_RESOURCE: &str = "netherwick.resource/ssd1306-attachment@1";
pub const SSD1306_FRAMEBUFFER_RESOURCE: &str = "netherwick.resource/ssd1306-framebuffer@1";

const FORM: &str = "form face {\n    renderer: presentation/renderer\n}\n";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ssd1306PresenterEvidence {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub i2c_base_id: String,
    pub attachment_id: String,
    pub framebuffer_resource_id: String,
    pub address: u8,
    pub observed_at_tick: u64,
    pub maximum_age_ticks: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ssd1306PresenterError {
    MissingIdentity,
    InvalidAddress,
    InvalidFreshness,
    StaleEvidence,
    Catalog,
    Plan,
    WrongPlan,
    PresentationIdentity,
    StaleRevision,
    Projection(Ssd1306ProjectionError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ssd1306Receipt {
    pub presentation_id: conduit_presentation::PresentationContentId,
    pub presentation_revision: u64,
    pub manifestation_id: conduit_presentation::ManifestationId,
    pub lifecycle: ManifestationLifecycle,
    pub manifestation_failure: Option<ManifestationFailure>,
    pub device_failure: Option<Ssd1306Failure>,
    pub presenter_implementation_id: ImplementationId,
    pub plan_id: conduit_core::PlanId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub i2c_base_id: String,
    pub attachment_id: String,
    pub frame: Ssd1306Frame,
    pub host_remains_available: bool,
    pub motion_safety_mutated: bool,
    pub physical_hil_claimed: bool,
}

pub struct Ssd1306Presenter {
    evidence: Ssd1306PresenterEvidence,
    plan: Plan,
    placement_id: conduit_core::PlacementId,
    session: Ssd1306Session,
    last_revision: u64,
}

impl Ssd1306Presenter {
    pub fn prepare(
        evidence: Ssd1306PresenterEvidence,
        now_tick: u64,
    ) -> Result<Self, Ssd1306PresenterError> {
        validate_evidence(&evidence, now_tick)?;
        let mut catalog = ProfileCatalog::new();
        catalog
            .insert(renderer_kind_definition())
            .map_err(|_| Ssd1306PresenterError::Catalog)?;
        let form = parse(FORM, &catalog).map_err(|_| Ssd1306PresenterError::Catalog)?;
        let host = advertisement(&evidence);
        let placements = default_placements(&form, core::slice::from_ref(&host))
            .map_err(|_| Ssd1306PresenterError::Plan)?;
        let plan =
            plan(&form, &[host], &placements, &[]).map_err(|_| Ssd1306PresenterError::Plan)?;
        validate_ssd1306_plan(&plan, &evidence)?;
        let placement_id = plan.fragments[0]
            .placements
            .iter()
            .find(|placement| {
                placement.implementation_id.as_str() == SSD1306_PRESENTER_IMPLEMENTATION
            })
            .ok_or(Ssd1306PresenterError::WrongPlan)?
            .placement_id
            .clone();
        let session = Ssd1306Session::new(evidence.address)
            .map_err(|_| Ssd1306PresenterError::InvalidAddress)?;
        Ok(Self {
            evidence,
            plan,
            placement_id,
            session,
            last_revision: 0,
        })
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    pub fn present<P: Ssd1306I2cProvider>(
        &mut self,
        presentation: &Presentation,
        provider: &mut P,
    ) -> Result<Ssd1306Receipt, Ssd1306PresenterError> {
        if presentation.revision <= self.last_revision {
            return Err(Ssd1306PresenterError::StaleRevision);
        }
        let face = presentation
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Host)
            .ok_or(Ssd1306PresenterError::PresentationIdentity)?
            .identity
            .clone();
        let frame =
            project_ssd1306_frame(presentation).map_err(Ssd1306PresenterError::Projection)?;
        let active = bind_active_play(
            &self.plan.plan_id,
            &self.evidence.host_id,
            &self.evidence.boot_id,
            presentation.revision,
        );
        let prepared = Manifestation::prepared(
            presentation,
            &self.plan,
            active,
            self.placement_id.clone(),
            face,
            self.evidence.attachment_id.clone(),
            SignId::from(format!(
                "netherwick/ssd1306/manifestation-prepared/{}",
                presentation.revision
            )),
        )
        .map_err(|_| Ssd1306PresenterError::PresentationIdentity)?;
        let device_failure = self.session.display(provider, &frame.framebuffer).err();
        let manifestation = if device_failure.is_none() {
            prepared.transition(
                ManifestationLifecycle::Available,
                SignId::from(format!(
                    "netherwick/ssd1306/manifestation-available/{}",
                    presentation.revision
                )),
            )
        } else {
            prepared.fail(
                ManifestationFailure::DeliveryFailed,
                SignId::from(format!(
                    "netherwick/ssd1306/manifestation-failed/{}",
                    presentation.revision
                )),
            )
        }
        .map_err(|_| Ssd1306PresenterError::PresentationIdentity)?;
        self.last_revision = presentation.revision;
        Ok(Ssd1306Receipt {
            presentation_id: presentation.identity.clone(),
            presentation_revision: presentation.revision,
            manifestation_id: manifestation.manifestation_id,
            lifecycle: manifestation.lifecycle,
            manifestation_failure: manifestation.failure,
            device_failure,
            presenter_implementation_id: ImplementationId::from(SSD1306_PRESENTER_IMPLEMENTATION),
            plan_id: self.plan.plan_id.clone(),
            host_id: self.evidence.host_id.clone(),
            boot_id: self.evidence.boot_id.clone(),
            i2c_base_id: self.evidence.i2c_base_id.clone(),
            attachment_id: self.evidence.attachment_id.clone(),
            frame,
            host_remains_available: true,
            motion_safety_mutated: false,
            physical_hil_claimed: false,
        })
    }
}

pub fn validate_ssd1306_plan(
    plan: &Plan,
    evidence: &Ssd1306PresenterEvidence,
) -> Result<(), Ssd1306PresenterError> {
    let placement = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.implementation_id.as_str() == SSD1306_PRESENTER_IMPLEMENTATION)
        .ok_or(Ssd1306PresenterError::WrongPlan)?;
    if placement.host_id != evidence.host_id
        || placement.boot_id != evidence.boot_id
        || placement.offer_generation != evidence.offer_generation
        || placement.host_operations.len() != 1
        || placement.host_operations[0].contract_id.as_str() != SSD1306_PRESENT_OPERATION
        || placement.resources.len() != 3
    {
        return Err(Ssd1306PresenterError::WrongPlan);
    }
    for (class, pool) in [
        (SSD1306_I2C_RESOURCE, evidence.i2c_base_id.as_str()),
        (SSD1306_ATTACHMENT_RESOURCE, evidence.attachment_id.as_str()),
        (
            SSD1306_FRAMEBUFFER_RESOURCE,
            evidence.framebuffer_resource_id.as_str(),
        ),
    ] {
        if !placement.resources.iter().any(|binding| {
            binding.class_id.as_str() == class
                && binding.pool_id.as_str() == pool
                && binding.units == 1
        }) {
            return Err(Ssd1306PresenterError::WrongPlan);
        }
    }
    Ok(())
}

fn advertisement(evidence: &Ssd1306PresenterEvidence) -> HostAdvertisement {
    let mut resources = vec![
        resource_offer(&evidence.i2c_base_id, SSD1306_I2C_RESOURCE, 1),
        resource_offer(&evidence.attachment_id, SSD1306_ATTACHMENT_RESOURCE, 1),
        resource_offer(
            &evidence.framebuffer_resource_id,
            SSD1306_FRAMEBUFFER_RESOURCE,
            1,
        ),
    ];
    resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let mut capability = renderer_offer(RendererRealizationOffer {
        capability_id: CapabilityId::from(SSD1306_PRESENTER_CAPABILITY),
        execution_profile_id: ExecutionProfileId::from(SSD1306_PRESENTER_PROFILE),
        implementation_id: ImplementationId::from(SSD1306_PRESENTER_IMPLEMENTATION),
        artifact_id: ArtifactId::from(SSD1306_PRESENTER_ARTIFACT),
        host_operation: HostOperationRequirement {
            contract_id: HostOperationContractId::from(SSD1306_PRESENT_OPERATION),
            target_kind: Some(conduit_core::kind_id("presentation/base/ssd1306-128x32@1")),
            maximum_in_flight: 1,
            maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
            maximum_output_bytes: MAX_RENDERER_VALUE_BYTES,
        },
        resource_requirement: resource_requirement(SSD1306_FRAMEBUFFER_RESOURCE, 1),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
        },
    });
    capability.resource_requirements = vec![
        resource_requirement(SSD1306_I2C_RESOURCE, 1),
        resource_requirement(SSD1306_ATTACHMENT_RESOURCE, 1),
        resource_requirement(SSD1306_FRAMEBUFFER_RESOURCE, 1),
    ];
    capability
        .resource_requirements
        .sort_by(|left, right| left.class_id.cmp(&right.class_id));
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: evidence.host_id.clone(),
        boot_id: evidence.boot_id.clone(),
        offer_generation: evidence.offer_generation,
        profile: HostProfileId::from(SSD1306_PRESENTER_PROFILE),
        resources,
        capabilities: vec![capability],
        planner_capabilities: Vec::new(),
    }
}

fn validate_evidence(
    evidence: &Ssd1306PresenterEvidence,
    now_tick: u64,
) -> Result<(), Ssd1306PresenterError> {
    if evidence.i2c_base_id.is_empty()
        || evidence.attachment_id.is_empty()
        || evidence.framebuffer_resource_id.is_empty()
    {
        return Err(Ssd1306PresenterError::MissingIdentity);
    }
    if !matches!(evidence.address, 0x3c | 0x3d) {
        return Err(Ssd1306PresenterError::InvalidAddress);
    }
    if evidence.maximum_age_ticks == 0 {
        return Err(Ssd1306PresenterError::InvalidFreshness);
    }
    if now_tick < evidence.observed_at_tick
        || now_tick - evidence.observed_at_tick > evidence.maximum_age_ticks
    {
        return Err(Ssd1306PresenterError::StaleEvidence);
    }
    Ok(())
}

#[cfg(test)]
#[path = "ssd1306_presenter_tests.rs"]
mod tests;
