//! Multi-surface native shell for the canonical Tour/Patchbay workspace.

mod lifecycle;
mod relayout;
mod scene;
#[cfg(test)]
mod tests;
mod transient;

use alloc::{format, string::String, vec, vec::Vec};

use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement,
    HostBaseId, HostId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    OfferGeneration, PROTOCOL_VERSION, PlacementId, Plan, SignId, bind_active_play, kind_id,
    resource_offer, resource_requirement,
};
use conduit_form::{ProfileCatalog, parse};
use conduit_planner::{default_placements, plan};
use conduit_presentation::{
    GraphicsScene, LayoutRect, MAX_RENDERER_VALUE_BYTES, Manifestation, ManifestationId,
    ManifestationLifecycle, Presentation, PresentationBasis, RendererRealizationOffer,
    renderer_kind_definition, renderer_offer,
};
use conduit_tour_model::{TOUR_WORKSPACE_SUBJECT, TourTransientKind};

use crate::{
    display::PixelTarget,
    native_compositor::{
        CompositionReceipt, CompositorAdmission, FrameReceipt, InputRoute,
        NATIVE_PRESENTER_IMPLEMENTATION, NativeCompositor, NativeCompositorError, RoutedKeyboard,
        RoutedPointer,
    },
    tour_product::TourProduct,
};
use lifecycle::empty_lifecycle_basis;
use scene::{ShellLayout, inspector_scene, status_scene, transient_scene};

pub const WORKSPACE_SURFACE: &str = "conduitos/shell/workspace";
pub const INSPECTOR_SURFACE: &str = "conduitos/shell/inspector";
pub const STATUS_SURFACE: &str = "conduitos/shell/status";
pub const TRANSIENT_SURFACE: &str = "conduitos/shell/transient";

const SURFACE_CLASS: &str = "presentation/surface";
const RENDERER_FORM: &str = concat!(
    "form shell {\n",
    "    workspace: presentation/renderer\n",
    "    inspector: presentation/renderer\n",
    "    status: presentation/renderer\n",
    "    transient: presentation/renderer\n",
    "}\n"
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Slot {
    Workspace,
    Inspector,
    Status,
    Transient,
}

impl Slot {
    const ALL: [Self; 4] = [
        Self::Workspace,
        Self::Inspector,
        Self::Status,
        Self::Transient,
    ];

    const fn gear(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Inspector => "inspector",
            Self::Status => "status",
            Self::Transient => "transient",
        }
    }

    const fn surface(self) -> &'static str {
        match self {
            Self::Workspace => WORKSPACE_SURFACE,
            Self::Inspector => INSPECTOR_SURFACE,
            Self::Status => STATUS_SURFACE,
            Self::Transient => TRANSIENT_SURFACE,
        }
    }
}

struct SurfaceState {
    slot: Slot,
    placement_id: PlacementId,
    admitted: bool,
    face_subject: Option<String>,
    manifestation_id: Option<ManifestationId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellPresentationReceipt {
    pub workspace: CompositionReceipt,
    pub inspector: Option<CompositionReceipt>,
    pub status: CompositionReceipt,
    pub frame: FrameReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellTransientReceipt {
    pub kind: TourTransientKind,
    pub parent_presentation_id: conduit_presentation::PresentationContentId,
    pub parent_manifestation_id: ManifestationId,
    pub transient: CompositionReceipt,
    pub frame: FrameReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellTransientDismissalReceipt {
    pub surface_id: String,
    pub manifestation_id: ManifestationId,
    pub frame: FrameReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellRelayoutReceipt {
    pub surface_id: String,
    pub previous_bounds: LayoutRect,
    pub current_bounds: LayoutRect,
    pub invalidated_manifestation_id: ManifestationId,
    pub current: CompositionReceipt,
    pub input_refused_while_invalidated: bool,
    pub frame: FrameReceipt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourShellError {
    Catalog,
    Plan,
    Identity,
    Scene,
    Compositor(NativeCompositorError),
}

impl TourShellError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "tour-shell-catalog-refused",
            Self::Plan => "tour-shell-plan-refused",
            Self::Identity => "tour-shell-identity-refused",
            Self::Scene => "tour-shell-scene-refused",
            Self::Compositor(error) => error.as_str(),
        }
    }
}

pub struct TourShellPresenter {
    plan: Plan,
    host_id: HostId,
    boot_id: BootId,
    display_base_id: HostBaseId,
    compositor: NativeCompositor,
    surfaces: Vec<SurfaceState>,
    play_sequence: u64,
    lifecycle_revision: u64,
    lifecycle_basis: PresentationBasis,
}

impl TourShellPresenter {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        profile_id: &str,
        image_id: &str,
        display_base_id: HostBaseId,
        surface_slots: u32,
    ) -> Result<Self, TourShellError> {
        if surface_slots < 4 {
            return Err(TourShellError::Identity);
        }
        let mut catalog = ProfileCatalog::new();
        catalog
            .insert(renderer_kind_definition())
            .map_err(|_| TourShellError::Catalog)?;
        let form = parse(RENDERER_FORM, &catalog).map_err(|_| TourShellError::Catalog)?;
        let implementation = ImplementationId::from(NATIVE_PRESENTER_IMPLEMENTATION);
        let host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
            offer_generation,
            profile: profile_id.into(),
            resources: vec![resource_offer(
                "conduitos/shell/surfaces",
                SURFACE_CLASS,
                surface_slots,
            )],
            capabilities: vec![renderer_offer(RendererRealizationOffer {
                capability_id: CapabilityId::from("conduitos/presenter/native-shell@1"),
                execution_profile_id: ExecutionProfileId::from("conduitos/native-product@1"),
                implementation_id: implementation.clone(),
                artifact_id: ArtifactId::from(image_id),
                host_operation: HostOperationRequirement {
                    contract_id: HostOperationContractId::from("conduit.host/present@1"),
                    target_kind: Some(kind_id("presentation/base/native-compositor@1")),
                    maximum_in_flight: 4,
                    maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
                    maximum_output_bytes: MAX_RENDERER_VALUE_BYTES,
                },
                resource_requirement: resource_requirement(SURFACE_CLASS, 1),
                limits: CapabilityLimits {
                    max_active_instances: 4,
                    max_queue_items: 4,
                    max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
                },
            })],
            planner_capabilities: Vec::new(),
        };
        let placements = default_placements(&form, core::slice::from_ref(&host))
            .map_err(|_| TourShellError::Plan)?;
        let plan = plan(&form, &[host], &placements, &[]).map_err(|_| TourShellError::Plan)?;
        let mut surfaces = Vec::new();
        for slot in Slot::ALL {
            let placement = plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .find(|placement| {
                    placement.gear_id.as_str().rsplit('/').next() == Some(slot.gear())
                })
                .ok_or(TourShellError::Plan)?;
            surfaces.push(SurfaceState {
                slot,
                placement_id: placement.placement_id.clone(),
                admitted: false,
                face_subject: None,
                manifestation_id: None,
            });
        }
        let admission = CompositorAdmission::new(
            host_id.clone(),
            boot_id.clone(),
            offer_generation,
            implementation,
            display_base_id.clone(),
            surfaces
                .iter()
                .map(|state| state.placement_id.clone())
                .collect(),
            Slot::ALL.iter().map(|slot| slot.surface().into()).collect(),
        )
        .map_err(TourShellError::Compositor)?;
        Ok(Self {
            plan,
            host_id,
            boot_id,
            display_base_id,
            compositor: NativeCompositor::admitted(admission),
            surfaces,
            play_sequence: 0,
            lifecycle_revision: 0,
            lifecycle_basis: empty_lifecycle_basis(),
        })
    }

    pub fn present(
        &mut self,
        tour: &TourProduct,
        display: &mut impl PixelTarget,
    ) -> Result<ShellPresentationReceipt, TourShellError> {
        let format = display
            .format()
            .validate()
            .map_err(NativeCompositorError::from)
            .map_err(TourShellError::Compositor)?;
        let width = u16::try_from(format.width).map_err(|_| TourShellError::Identity)?;
        let height = u16::try_from(format.height).map_err(|_| TourShellError::Identity)?;
        let layout = ShellLayout::new(width, height)?;
        let state = tour.controller().state();
        let workspace_presentation = state
            .workspace_presentation()
            .map_err(|_| TourShellError::Identity)?;
        let workspace_scene = tour
            .scene(width, height)
            .map_err(|_| TourShellError::Scene)?;
        let workspace = self.present_surface(
            Slot::Workspace,
            &workspace_presentation,
            TOUR_WORKSPACE_SUBJECT,
            layout.workspace,
            0,
            &workspace_scene,
        )?;
        let status_presentation = state
            .status_presentation_with_basis(
                self.lifecycle_revision
                    .checked_add(u64::from(state.revision))
                    .ok_or(TourShellError::Identity)?,
                self.lifecycle_basis.clone(),
            )
            .map_err(|_| TourShellError::Identity)?;
        let status = self.present_surface(
            Slot::Status,
            &status_presentation,
            "tour/status",
            layout.status,
            1,
            &status_scene(layout.status, &status_presentation)?,
        )?;
        let inspector = if let Some(presentation) = state
            .inspector_presentation()
            .map_err(|_| TourShellError::Identity)?
        {
            let face = presentation
                .subjects
                .iter()
                .find(|subject| subject.identity.ends_with("/inspection"))
                .ok_or(TourShellError::Identity)?
                .identity
                .clone();
            Some(self.present_surface(
                Slot::Inspector,
                &presentation,
                &face,
                layout.inspector,
                2,
                &inspector_scene(layout.inspector, &presentation)?,
            )?)
        } else {
            self.dismiss(Slot::Inspector)?;
            None
        };
        let frame = self
            .compositor
            .compose_frame(display)
            .map_err(TourShellError::Compositor)?;
        if self.compositor.focused_surface().is_none() {
            self.compositor
                .focus_surface(WORKSPACE_SURFACE)
                .map_err(TourShellError::Compositor)?;
        }
        Ok(ShellPresentationReceipt {
            workspace,
            inspector,
            status,
            frame,
        })
    }

    pub fn route_pointer(
        &mut self,
        x: u32,
        y: u32,
        activate: bool,
    ) -> Result<InputRoute<RoutedPointer>, TourShellError> {
        self.compositor
            .route_pointer(x, y, activate)
            .map_err(TourShellError::Compositor)
    }

    pub fn validate_pointer_route(&self, route: &RoutedPointer) -> Result<(), TourShellError> {
        self.compositor
            .validate_pointer_route(route)
            .map_err(TourShellError::Compositor)
    }

    pub fn route_keyboard(&self) -> Result<InputRoute<RoutedKeyboard>, TourShellError> {
        let Some(surface) = self.compositor.focused_surface() else {
            return Ok(InputRoute::NoTarget);
        };
        let Some(expected) = self
            .surfaces
            .iter()
            .find(|state| state.slot.surface() == surface)
            .and_then(|state| state.manifestation_id.as_ref())
        else {
            return Ok(InputRoute::NoTarget);
        };
        self.compositor
            .route_keyboard(expected)
            .map_err(TourShellError::Compositor)
    }

    fn present_surface(
        &mut self,
        slot: Slot,
        presentation: &Presentation,
        face_subject: &str,
        bounds: LayoutRect,
        z: u8,
        scene: &GraphicsScene,
    ) -> Result<CompositionReceipt, TourShellError> {
        let index = self
            .surfaces
            .iter()
            .position(|state| state.slot == slot)
            .ok_or(TourShellError::Identity)?;
        if self.surfaces[index]
            .face_subject
            .as_deref()
            .is_some_and(|current| current != face_subject)
        {
            self.dismiss(slot)?;
        }
        if !self.surfaces[index].admitted {
            self.compositor
                .admit_surface(slot.surface(), bounds, z)
                .map_err(TourShellError::Compositor)?;
            self.surfaces[index].admitted = true;
        } else {
            self.compositor
                .place_surface(slot.surface(), bounds, z)
                .map_err(TourShellError::Compositor)?;
        }
        self.play_sequence = self
            .play_sequence
            .checked_add(1)
            .ok_or(TourShellError::Identity)?;
        let active = bind_active_play(
            &self.plan.plan_id,
            &self.host_id,
            &self.boot_id,
            self.play_sequence,
        );
        let manifestation = Manifestation::prepared(
            presentation,
            &self.plan,
            active,
            self.surfaces[index].placement_id.clone(),
            face_subject.into(),
            slot.surface().into(),
            SignId::from(format!(
                "conduitos/shell/{}/prepared/{}",
                slot.gear(),
                self.play_sequence
            )),
        )
        .and_then(|value| {
            value.transition(
                ManifestationLifecycle::Available,
                SignId::from(format!(
                    "conduitos/shell/{}/available/{}",
                    slot.gear(),
                    self.play_sequence
                )),
            )
        })
        .map_err(|_| TourShellError::Identity)?;
        let receipt = self
            .compositor
            .update_surface(
                presentation,
                &manifestation,
                &self.plan,
                slot.surface(),
                &self.display_base_id,
                scene,
            )
            .map_err(TourShellError::Compositor)?
            .clone();
        self.surfaces[index].face_subject = Some(face_subject.into());
        self.surfaces[index].manifestation_id = Some(receipt.manifestation_id.clone());
        Ok(receipt)
    }

    fn dismiss(&mut self, slot: Slot) -> Result<(), TourShellError> {
        let state = self
            .surfaces
            .iter_mut()
            .find(|state| state.slot == slot)
            .ok_or(TourShellError::Identity)?;
        if state.admitted {
            self.compositor
                .remove_surface(slot.surface())
                .map_err(TourShellError::Compositor)?;
            state.admitted = false;
            state.face_subject = None;
            state.manifestation_id = None;
        }
        Ok(())
    }
}
