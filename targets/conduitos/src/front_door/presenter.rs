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
    LayoutRect, MAX_RENDERER_VALUE_BYTES, Manifestation, ManifestationId, ManifestationLifecycle,
    PresentationRole, RendererRealizationOffer, renderer_kind_definition, renderer_offer,
};

use super::{Error as FrontDoorError, FrontDoor};
use crate::{
    display::PixelTarget,
    native_compositor::{
        CompositionReceipt, CompositorAdmission, InputRoute, NATIVE_PRESENTER_IMPLEMENTATION,
        NativeCompositor, NativeCompositorError, RoutedKeyboard, RoutedPointer,
    },
};

const SURFACE_CLASS: &str = "presentation/surface";
const SURFACE_ID: &str = "conduitos/front-door/surface/0";
const RENDERER_FORM: &str = "form face {\n    renderer: presentation/renderer\n}\n";

pub struct FrontDoorPresenter {
    plan: Plan,
    placement_id: conduit_core::PlacementId,
    display_base_id: HostBaseId,
    host_id: HostId,
    boot_id: BootId,
    last_revision: u64,
    compositor: NativeCompositor,
    surface_admitted: bool,
    last_manifestation_id: Option<ManifestationId>,
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
    pub const fn compositor_frame_sequence(&self) -> u64 {
        self.compositor.frame_sequence()
    }

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
        let admission = CompositorAdmission::new(
            host_id.clone(),
            boot_id.clone(),
            offer_generation,
            implementation_id.clone(),
            display_base_id.clone(),
            vec![placement_id.clone()],
            vec![SURFACE_ID.into()],
        )
        .map_err(PresenterError::Compositor)?;
        Ok(Self {
            plan,
            placement_id,
            display_base_id,
            host_id,
            boot_id,
            last_revision: 0,
            compositor: NativeCompositor::admitted(admission),
            surface_admitted: false,
            last_manifestation_id: None,
        })
    }

    pub fn route_pointer(
        &mut self,
        display_x: u32,
        display_y: u32,
        activate: bool,
    ) -> Result<InputRoute<RoutedPointer>, PresenterError> {
        self.compositor
            .route_pointer(display_x, display_y, activate)
            .map_err(PresenterError::Compositor)
    }

    pub fn validate_pointer_route(&self, route: &RoutedPointer) -> Result<(), PresenterError> {
        self.compositor
            .validate_pointer_route(route)
            .map_err(PresenterError::Compositor)
    }

    pub fn route_keyboard(&self) -> Result<InputRoute<RoutedKeyboard>, PresenterError> {
        let expected = self
            .last_manifestation_id
            .as_ref()
            .ok_or(PresenterError::Identity)?;
        self.compositor
            .route_keyboard(expected)
            .map_err(PresenterError::Compositor)
    }

    /// Relinquish retained scanout storage before another shell presentation
    /// becomes the active owner of the finite native display service.
    pub fn suspend(&mut self) -> Result<(), PresenterError> {
        if self.surface_admitted {
            self.compositor
                .remove_surface(SURFACE_ID)
                .map_err(PresenterError::Compositor)?;
            self.surface_admitted = false;
            self.last_manifestation_id = None;
            self.last_revision = 0;
        }
        Ok(())
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
        let scene = front_door
            .scene(display)
            .map_err(PresenterError::FrontDoor)?;
        let format = display
            .format()
            .validate()
            .map_err(NativeCompositorError::from)
            .map_err(PresenterError::Compositor)?;
        let bounds = LayoutRect {
            x: 0,
            y: 0,
            width: u16::try_from(format.width).map_err(|_| PresenterError::Identity)?,
            height: u16::try_from(format.height).map_err(|_| PresenterError::Identity)?,
        };
        if self.surface_admitted {
            self.compositor
                .place_surface(SURFACE_ID, bounds, 0)
                .map_err(PresenterError::Compositor)?;
        } else {
            self.compositor
                .admit_surface(SURFACE_ID, bounds, 0)
                .map_err(PresenterError::Compositor)?;
            self.surface_admitted = true;
        }
        let receipt = self
            .compositor
            .update_surface(
                &presentation,
                &manifestation,
                &self.plan,
                SURFACE_ID,
                &self.display_base_id,
                &scene,
            )
            .map_err(PresenterError::Compositor)?
            .clone();
        self.compositor
            .compose_frame(display)
            .map_err(PresenterError::Compositor)?;
        if self.compositor.focused_surface().is_none() {
            self.compositor
                .focus_surface(SURFACE_ID)
                .map_err(PresenterError::Compositor)?;
        }
        self.last_manifestation_id = Some(receipt.manifestation_id.clone());
        self.last_revision = presentation.revision;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use conduit_core::{CheckedFormId, SourceDocumentId};
    use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

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
        let mut door = door();
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
        assert_eq!(presenter.compositor_frame_sequence(), 1);
        let pointer = match presenter.route_pointer(100, 50, true).unwrap() {
            InputRoute::Delivered(route) => route,
            InputRoute::NoTarget => panic!("front-door surface must receive pointer input"),
        };
        assert_eq!((pointer.local_x, pointer.local_y), (100, 50));
        presenter.validate_pointer_route(&pointer).unwrap();
        assert!(matches!(
            presenter.route_keyboard().unwrap(),
            InputRoute::Delivered(_)
        ));
        assert_eq!(
            presenter.present(&door, &mut display),
            Err(PresenterError::StaleRevision)
        );
        door.accept(
            KeyEvent::new(43, KeyTransition::Pressed, KeyModifiers::from_bits(0)).unwrap(),
            1,
        )
        .unwrap();
        let next = presenter.present(&door, &mut display).unwrap();
        assert_ne!(next.manifestation_id, receipt.manifestation_id);
        assert_eq!(presenter.compositor_frame_sequence(), 2);
        assert_eq!(
            presenter.validate_pointer_route(&pointer),
            Err(PresenterError::Compositor(
                NativeCompositorError::StaleSurfaceBinding
            ))
        );

        presenter.suspend().unwrap();
        assert_eq!(presenter.route_keyboard(), Err(PresenterError::Identity));
        presenter.present(&door, &mut display).unwrap();
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
