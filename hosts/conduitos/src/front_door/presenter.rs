//! Exact planned native Presenter realization for zero-Body WORLD revisions.

#[cfg(test)]
use alloc::format;
use alloc::{vec, vec::Vec};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement,
    HostBaseId, HostId, HostOperationContractId, HostOperationRequirement, HostProfileId,
    ImplementationId, OfferGeneration, PROTOCOL_VERSION, Plan, SignId, bind_active_play, kind_id,
    resource_offer, resource_requirement,
};
use conduit_form::{ProfileCatalog, parse};
use conduit_planner::{default_placements, plan};
use conduit_presentation::{
    MAX_RENDERER_VALUE_BYTES, Manifestation, ManifestationLifecycle, PresentationRole,
    RendererRealizationOffer, renderer_kind_definition, renderer_offer,
};

use super::{Error as FrontDoorError, FrontDoor};
use crate::{
    display::PixelTarget,
    native_compositor::{
        CompositionReceipt, CompositorAdmission, NATIVE_PRESENTER_IMPLEMENTATION, NativeCompositor,
        NativeCompositorError,
    },
};

const SURFACE_CLASS: &str = "presentation/surface";
const SURFACE_ID: &str = "conduitos/front-door/surface/0";
const RENDERER_FORM: &str = "form 0\n\nface {\n    renderer: presentation/renderer\n}\n";

pub struct FrontDoorPresenter {
    plan: Plan,
    placement_id: conduit_core::PlacementId,
    display_base_id: HostBaseId,
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    implementation_id: ImplementationId,
    last_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenterError {
    Catalog,
    Plan,
    Identity,
    StaleRevision,
    FrontDoor(FrontDoorError),
    Compositor(NativeCompositorError),
}

impl PresenterError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "front-door-presenter-catalog-refused",
            Self::Plan => "front-door-presenter-plan-refused",
            Self::Identity => "front-door-presenter-identity-refused",
            Self::StaleRevision => "front-door-presentation-revision-stale",
            Self::FrontDoor(error) => error.as_str(),
            Self::Compositor(error) => error.as_str(),
        }
    }
}

impl FrontDoorPresenter {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        profile_id: &str,
        image_id: &str,
        display_base_id: HostBaseId,
        surface_slots: u32,
    ) -> Result<Self, PresenterError> {
        if surface_slots == 0 {
            return Err(PresenterError::Identity);
        }
        let mut catalog = ProfileCatalog::new();
        catalog
            .insert(renderer_kind_definition())
            .map_err(|_| PresenterError::Catalog)?;
        let form = parse(RENDERER_FORM, &catalog).map_err(|_| PresenterError::Catalog)?;
        let implementation_id = ImplementationId::from(NATIVE_PRESENTER_IMPLEMENTATION);
        let host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
            offer_generation,
            profile: HostProfileId::from(profile_id),
            resources: vec![resource_offer(SURFACE_ID, SURFACE_CLASS, surface_slots)],
            capabilities: vec![renderer_offer(RendererRealizationOffer {
                capability_id: CapabilityId::from("conduitos/presenter/native-front-door@1"),
                execution_profile_id: ExecutionProfileId::from("conduitos/native-product@1"),
                implementation_id: implementation_id.clone(),
                artifact_id: ArtifactId::from(image_id),
                host_operation: HostOperationRequirement {
                    contract_id: HostOperationContractId::from("conduit.host/present@1"),
                    target_kind: Some(kind_id("presentation/base/native-compositor@1")),
                    maximum_in_flight: 1,
                    maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
                    maximum_output_bytes: MAX_RENDERER_VALUE_BYTES,
                },
                resource_requirement: resource_requirement(SURFACE_CLASS, 1),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
                },
            })],
            planner_capabilities: Vec::new(),
        };
        let placements = default_placements(&form, core::slice::from_ref(&host))
            .map_err(|_| PresenterError::Plan)?;
        let plan = plan(&form, &[host], &placements, &[]).map_err(|_| PresenterError::Plan)?;
        let placement_id = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .next()
            .ok_or(PresenterError::Plan)?
            .placement_id
            .clone();
        Ok(Self {
            plan,
            placement_id,
            display_base_id,
            host_id,
            boot_id,
            offer_generation,
            implementation_id,
            last_revision: 0,
        })
    }

    pub fn present(
        &mut self,
        front_door: &FrontDoor,
        display: &mut impl PixelTarget,
    ) -> Result<CompositionReceipt, PresenterError> {
        let presentation = front_door
            .presentation()
            .map_err(PresenterError::FrontDoor)?;
        if presentation.revision <= self.last_revision {
            return Err(PresenterError::StaleRevision);
        }
        let face_subject = presentation
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Host)
            .ok_or(PresenterError::Identity)?
            .identity
            .clone();
        let active = bind_active_play(
            &self.plan.plan_id,
            &self.host_id,
            &self.boot_id,
            presentation.revision,
        );
        let manifestation = Manifestation::prepared(
            &presentation,
            &self.plan,
            active,
            self.placement_id.clone(),
            face_subject,
            SURFACE_ID.into(),
            SignId::from("conduitos/front-door/manifestation-prepared"),
        )
        .and_then(|value| {
            value.transition(
                ManifestationLifecycle::Available,
                SignId::from("conduitos/front-door/manifestation-available"),
            )
        })
        .map_err(|_| PresenterError::Identity)?;
        let admission = CompositorAdmission::new(
            self.host_id.clone(),
            self.boot_id.clone(),
            self.offer_generation,
            self.implementation_id.clone(),
            self.display_base_id.clone(),
            vec![self.placement_id.clone()],
            vec![SURFACE_ID.into()],
        )
        .map_err(PresenterError::Compositor)?;
        let scene = front_door
            .scene(display)
            .map_err(PresenterError::FrontDoor)?;
        let mut compositor = NativeCompositor::admitted(admission, display);
        let receipt = compositor
            .compose(
                &presentation,
                &manifestation,
                &self.plan,
                SURFACE_ID,
                &self.display_base_id,
                &scene,
            )
            .map_err(PresenterError::Compositor)?
            .clone();
        self.last_revision = presentation.revision;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use conduit_core::{CheckedFormId, SourceDocumentId};

    use super::*;
    use crate::display::{DisplayError, DisplayFormat};

    struct MemoryDisplay {
        pixels: Vec<u32>,
        lost: bool,
    }

    impl MemoryDisplay {
        fn available() -> Self {
            Self {
                pixels: vec![0; 640 * 480],
                lost: false,
            }
        }
    }

    impl PixelTarget for MemoryDisplay {
        fn format(&self) -> DisplayFormat {
            DisplayFormat {
                width: 640,
                height: 480,
                pitch: 2_560,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }

        fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
            if self.lost {
                return Err(DisplayError::Lost);
            }
            let index = usize::try_from(y * 640 + x).unwrap();
            self.pixels[index] = pixel;
            Ok(())
        }
    }

    fn door() -> FrontDoor {
        FrontDoor::new(
            HostId::from("host"),
            BootId::from("boot"),
            OfferGeneration(4),
            "profile:one",
            "build:one",
            "image:one",
            SourceDocumentId::from("source"),
            CheckedFormId::from("checked"),
            6,
            false,
        )
    }

    fn presenter() -> FrontDoorPresenter {
        FrontDoorPresenter::prepare(
            HostId::from("host"),
            BootId::from("boot"),
            OfferGeneration(4),
            "profile:one",
            "image:one",
            HostBaseId::from("display/base"),
            2,
        )
        .unwrap()
    }

    #[test]
    fn zero_body_revision_crosses_exact_native_manifestation() {
        let door = door();
        let mut presenter = presenter();
        let mut display = MemoryDisplay::available();
        let receipt = presenter.present(&door, &mut display).unwrap();
        assert_eq!(
            receipt.presentation_id,
            door.presentation().unwrap().identity
        );
        assert_eq!(receipt.host_id.as_str(), "host");
        assert_eq!(receipt.boot_id.as_str(), "boot");
        assert_eq!(receipt.offer_generation, OfferGeneration(4));
        assert_eq!(receipt.display_base_id.as_str(), "display/base");
        assert!(receipt.display.pixels_written > 0);
        assert_eq!(
            presenter.present(&door, &mut display),
            Err(PresenterError::StaleRevision)
        );
    }

    #[test]
    fn graphical_and_linear_presenters_consume_the_same_portable_action_truth() {
        let door = door();
        let portable = door.presentation().unwrap();
        let mut display = MemoryDisplay::available();
        let graphical = presenter().present(&door, &mut display).unwrap();
        let linear = crate::linear_presenter::LinearPresenter::prepare(
            HostId::from("host"),
            BootId::from("boot"),
            OfferGeneration(4),
            "profile:one",
            "image:one",
        )
        .unwrap()
        .present(&portable)
        .unwrap();
        assert_eq!(graphical.presentation_id, portable.identity);
        assert_eq!(linear.presentation.presentation_id, portable.identity);
        for action in &portable.actions {
            assert!(linear.presentation.lines.iter().any(|line| {
                line.contains(&format!("id={:?}", action.identity))
                    && line.contains(&format!("intent={:?}", action.intent))
            }));
        }
    }

    #[test]
    fn missing_surface_and_lost_display_refuse_distinctly() {
        assert!(matches!(
            FrontDoorPresenter::prepare(
                HostId::from("host"),
                BootId::from("boot"),
                OfferGeneration(4),
                "profile:one",
                "image:one",
                HostBaseId::from("display/base"),
                0,
            ),
            Err(PresenterError::Identity)
        ));
        let door = door();
        let mut presenter = presenter();
        let mut display = MemoryDisplay::available();
        display.lost = true;
        assert_eq!(
            presenter.present(&door, &mut display),
            Err(PresenterError::Compositor(NativeCompositorError::Display(
                DisplayError::Lost
            )))
        );
    }
}
