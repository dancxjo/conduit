//! Exact admitted linear Presenter realization for non-graphical Hosts.

use alloc::{vec, vec::Vec};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement,
    HostId, HostOperationContractId, HostOperationRequirement, HostProfileId, ImplementationId,
    OfferGeneration, PROTOCOL_VERSION, Plan, SignId, bind_active_play, kind_id, resource_offer,
    resource_requirement,
};
use conduit_form::{ProfileCatalog, parse};
use conduit_planner::{default_placements, plan};
use conduit_presentation::{
    LinearPresentation, MAX_RENDERER_VALUE_BYTES, Manifestation, ManifestationLifecycle,
    Presentation, PresentationRole, RendererRealizationOffer, render_linear_presentation,
    renderer_kind_definition, renderer_offer,
};

pub const IMPLEMENTATION: &str = "presenter/linear-serial@1";
pub const CAPABILITY: &str = "conduitos/presenter/linear-serial@1";
pub const BASE_ID: &str = "conduitos/base/pl011-serial/0";
const RESOURCE_CLASS: &str = "presentation/linear-slot";
const RESOURCE_ID: &str = "conduitos/presentation/linear/0";
const FORM: &str = "form face {\n    renderer: presentation/renderer\n}\n";

#[derive(Debug, Clone)]
pub struct LinearReceipt {
    pub presentation: LinearPresentation,
    pub manifestation_id: conduit_presentation::ManifestationId,
    pub presenter_implementation_id: ImplementationId,
    pub plan_id: conduit_core::PlanId,
}

pub struct LinearPresenter {
    plan: Plan,
    placement_id: conduit_core::PlacementId,
    host_id: HostId,
    boot_id: BootId,
    implementation_id: ImplementationId,
    last_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinearPresenterError {
    Catalog,
    Plan,
    Identity,
    StaleRevision,
    Render,
}

impl LinearPresenter {
    pub fn prepare(
        host_id: HostId,
        boot_id: BootId,
        generation: OfferGeneration,
        profile_id: &str,
        image_id: &str,
    ) -> Result<Self, LinearPresenterError> {
        let mut catalog = ProfileCatalog::new();
        catalog
            .insert(renderer_kind_definition())
            .map_err(|_| LinearPresenterError::Catalog)?;
        let form = parse(FORM, &catalog).map_err(|_| LinearPresenterError::Catalog)?;
        let implementation_id = ImplementationId::from(IMPLEMENTATION);
        let host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
            offer_generation: generation,
            profile: HostProfileId::from(profile_id),
            resources: vec![resource_offer(RESOURCE_ID, RESOURCE_CLASS, 1)],
            capabilities: vec![renderer_offer(RendererRealizationOffer {
                capability_id: CapabilityId::from(CAPABILITY),
                execution_profile_id: ExecutionProfileId::from("conduitos/linear-product@1"),
                implementation_id: implementation_id.clone(),
                artifact_id: ArtifactId::from(image_id),
                host_operation: HostOperationRequirement {
                    contract_id: HostOperationContractId::from("conduit.host/present@1"),
                    target_kind: Some(kind_id("presentation/base/linear-serial@1")),
                    maximum_in_flight: 1,
                    maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
                    maximum_output_bytes: MAX_RENDERER_VALUE_BYTES,
                },
                resource_requirement: resource_requirement(RESOURCE_CLASS, 1),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
                },
            })],
            planner_capabilities: Vec::new(),
        };
        let placements = default_placements(&form, core::slice::from_ref(&host))
            .map_err(|_| LinearPresenterError::Plan)?;
        let plan =
            plan(&form, &[host], &placements, &[]).map_err(|_| LinearPresenterError::Plan)?;
        let placement_id = plan.fragments[0].placements[0].placement_id.clone();
        Ok(Self {
            plan,
            placement_id,
            host_id,
            boot_id,
            implementation_id,
            last_revision: 0,
        })
    }

    pub fn present(
        &mut self,
        presentation: &Presentation,
    ) -> Result<LinearReceipt, LinearPresenterError> {
        if presentation.revision <= self.last_revision {
            return Err(LinearPresenterError::StaleRevision);
        }
        let face = presentation
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Host)
            .ok_or(LinearPresenterError::Identity)?
            .identity
            .clone();
        let active = bind_active_play(
            &self.plan.plan_id,
            &self.host_id,
            &self.boot_id,
            presentation.revision,
        );
        let manifestation = Manifestation::prepared(
            presentation,
            &self.plan,
            active,
            self.placement_id.clone(),
            face,
            BASE_ID.into(),
            SignId::from("conduitos/linear/manifestation-prepared"),
        )
        .and_then(|value| {
            value.transition(
                ManifestationLifecycle::Available,
                SignId::from("conduitos/linear/manifestation-available"),
            )
        })
        .map_err(|_| LinearPresenterError::Identity)?;
        let linear =
            render_linear_presentation(presentation).map_err(|_| LinearPresenterError::Render)?;
        self.last_revision = presentation.revision;
        Ok(LinearReceipt {
            presentation: linear,
            manifestation_id: manifestation.manifestation_id,
            presenter_implementation_id: self.implementation_id.clone(),
            plan_id: self.plan.plan_id.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front_door::FrontDoor;
    use conduit_core::{CheckedFormId, SourceDocumentId};

    fn presentation() -> Presentation {
        FrontDoor::new(
            HostId::from("host"),
            BootId::from("boot"),
            OfferGeneration(1),
            "profile:one",
            "build:one",
            "image:one",
            SourceDocumentId::from("source"),
            CheckedFormId::from("checked"),
            5,
            false,
        )
        .presentation()
        .unwrap()
    }

    #[test]
    fn realizes_the_portable_zero_body_presentation_through_an_exact_plan() {
        let value = presentation();
        let mut presenter = LinearPresenter::prepare(
            HostId::from("host"),
            BootId::from("boot"),
            OfferGeneration(1),
            "profile:one",
            "image:one",
        )
        .unwrap();
        let receipt = presenter.present(&value).unwrap();
        assert_eq!(
            receipt.presenter_implementation_id,
            ImplementationId::from(IMPLEMENTATION)
        );
        assert!(
            receipt
                .presentation
                .lines
                .iter()
                .any(|line| line.contains("BODY NONE"))
        );
        assert!(
            receipt
                .presentation
                .lines
                .iter()
                .any(|line| line.contains("intent=\"conduit.intent/open@1\""))
        );
        assert!(
            receipt
                .presentation
                .lines
                .iter()
                .any(|line| line.contains("intent=\"conduit.intent/birth@1\"")
                    && line.contains("unavailable"))
        );
        assert!(matches!(
            presenter.present(&value),
            Err(LinearPresenterError::StaleRevision)
        ));
    }
}
