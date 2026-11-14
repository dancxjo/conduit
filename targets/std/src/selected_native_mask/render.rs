//! Direct semantic-Face rendering through the existing scene/compositor/kernel.
use super::*;
use crate::kernel_preparation::KernelResourceLedger;
use conduitos::display::PixelTarget;
use conduitos::{
    native_compositor::{CompositorAdmission, NativeCompositor},
    native_face_scene::PreparedStaticNativeFaceScene,
    product_bases::{
        native_mask_plan_backing_bytes, ActiveNativeSurfacePossession,
        NativeMaskInteractionSession, NativeSurfacePossessionReceipt, PreparedNativeMaskPlay,
    },
};

mod completion;
mod interactive;
use interactive::PreparedSelectedScene;
pub use interactive::{SelectedNativeInteraction, SelectedNativeMaskInput};

pub(super) struct PreparedReadOnlyBinding {
    plan: PlannedMaskPlot,
    compositor: NativeCompositor,
    base: HostBaseId,
    width: u16,
    height: u16,
}
impl PreparedReadOnlyBinding {
    pub(super) fn prepare(
        plan: PlannedMaskPlot,
        provider: &NativeSurfaceProvider,
        raster: PreparedCompositorBacking,
    ) -> Result<Self, String> {
        let fragment = plan
            .plan
            .fragments
            .first()
            .ok_or("native Mask fragment absent")?;
        let renderer = plan.show_placement();
        let admission = CompositorAdmission::new(
            fragment.host_id.clone(),
            fragment.boot_id.clone(),
            fragment.offer_generation,
            renderer.implementation_id.clone(),
            provider.entry.base_id.clone(),
            vec![renderer.placement_id.clone()],
            vec![SURFACE.into()],
        )
        .map_err(|error| format!("native Mask compositor admission: {error:?}"))?;
        let (width, height) = raster.geometry();
        let compositor = raster
            .bind(admission)
            .map_err(|error| format!("native Mask raster binding: {error:?}"))?;
        Ok(Self {
            plan,
            compositor,
            base: provider.entry.base_id.clone(),
            width,
            height,
        })
    }
    pub(super) fn plan(&self) -> &PlannedMaskPlot {
        &self.plan
    }
    pub(super) fn metadata_bytes(&self, fixed_raster: usize) -> Result<usize, String> {
        native_mask_plan_backing_bytes(&self.plan)
            .and_then(|bytes| bytes.checked_add(self.compositor.ordinary_retained_backing_bytes()?))
            .and_then(|bytes| bytes.checked_add(self.base.0.capacity()))
            .and_then(|bytes| bytes.checked_sub(fixed_raster))
            .ok_or_else(|| "native Mask unsupported or overflowing binding owners".into())
    }
    pub(super) fn bind(
        self,
        kernel: UnboundNativeMaskStorage,
    ) -> Result<BoundReadOnlyMask, String> {
        let storage = kernel
            .bind(&self.plan)
            .map_err(|error| format!("native Mask exact kernel binding: {error:?}"))?;
        Ok(BoundReadOnlyMask {
            plan: self.plan,
            compositor: self.compositor,
            base: self.base,
            width: self.width,
            height: self.height,
            storage: Some(storage),
            session: None,
            frame: None,
            failed: false,
            last_play: None,
            scene: None,
        })
    }
}
struct FrameCustody {
    composition: CompositionReceipt,
    possession: NativeSurfacePossessionReceipt,
    _heap: ImageMemoryLease,
    pool: BaseBackingReservation,
}
struct ActiveFrameCustody {
    composition: CompositionReceipt,
    possession: ActiveNativeSurfacePossession,
    _heap: ImageMemoryLease,
    pool: BaseBackingReservation,
}
/// Complete local evidence plus its existing managed-memory custody. Dropping
/// this owner is not retirement and does not release the ledger pool counter.
pub struct SelectedNativeMaskEvidence {
    evidence: RetiredNativeMaskEvidence,
    frame: FrameCustody,
}
impl SelectedNativeMaskEvidence {
    pub fn evidence(&self) -> &RetiredNativeMaskEvidence {
        &self.evidence
    }
    pub fn composition(&self) -> &CompositionReceipt {
        &self.frame.composition
    }
    pub fn possession(&self) -> &NativeSurfacePossessionReceipt {
        &self.frame.possession
    }
    pub(super) fn matches_host(&self, host: &StdHost) -> bool {
        self.frame.pool.matches_ledger(&host.kernel_resources)
            && self.frame.pool.matches_registry(&host.base_registry)
    }
    pub(super) fn retire(self, ledger: &mut KernelResourceLedger) -> Result<(), String> {
        let Self { evidence, frame } = self;
        let FrameCustody {
            composition,
            possession,
            _heap,
            pool,
        } = frame;
        drop(evidence);
        drop(composition);
        drop(possession);
        drop(_heap);
        ledger
            .release_base_backing(pool)
            .map_err(|(_, error)| error)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedNativeMaskFrame {
    pub face_revision: u64,
    pub play_sequence: u64,
    pub scanout_frame_sequence: u64,
}
pub(super) struct BoundReadOnlyMask {
    plan: PlannedMaskPlot,
    compositor: NativeCompositor,
    storage: Option<RetainedNativeMaskStorage>,
    session: Option<NativeMaskInteractionSession>,
    frame: Option<ActiveFrameCustody>,
    base: HostBaseId,
    width: u16,
    height: u16,
    failed: bool,
    last_play: Option<u64>,
    scene: Option<conduitos::native_face_scene::NativeFaceScene>,
}
impl BoundReadOnlyMask {
    #[cfg(test)]
    pub(super) fn session_absent_for_test(&self) -> bool {
        self.session.is_none() && self.frame.is_none()
    }
    pub(super) fn plan(&self) -> &PlannedMaskPlot {
        &self.plan
    }
    pub(super) fn show(&self) -> Option<&conduit_face::MaskShow> {
        self.session
            .as_ref()
            .filter(|_| !self.failed)
            .map(|session| session.show())
    }
    pub(super) fn can_retire_plan(&self) -> bool {
        !self.failed && self.session.is_none() && self.frame.is_none() && self.storage.is_some()
    }
    pub(super) fn healthy(&self) -> bool {
        !self.failed
    }
    pub(super) fn kernel_metadata_bytes(&self) -> Result<usize, String> {
        self.storage
            .as_ref()
            .map(|storage| storage.retained_metadata_bytes())
            .ok_or_else(|| "native Mask kernel metadata absent".into())
    }
    // These independent Host/provider/encounter/route/Face/output custody
    // witnesses must remain explicit at the actual typed Host Call boundary.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render(
        &mut self,
        host: &mut StdHost,
        provider: &NativeSurfaceProvider,
        encounter: &impl NativeMaskEncounter,
        route: &RemoteOwnerMaskRouteSeal,
        face: &Face,
        play_sequence: u64,
        front_subject: &str,
        output: &mut conduit_linux_display::Scanout<'_>,
        interaction_bytes: Option<()>,
    ) -> Result<SelectedNativeMaskFrame, String> {
        if self.failed || self.session.is_some() || self.frame.is_some() || self.storage.is_none() {
            return Err(
                "native Mask requires previous complete evidence retirement/handoff".into(),
            );
        }
        if self.last_play.is_some_and(|last| play_sequence <= last) {
            return Err("native Mask child Play sequence is stale".into());
        }
        let interactive = self.interactive();
        if interactive != interaction_bytes.is_some() {
            return Err(
                "selected input profile requires its qualified per-frame preparation envelope"
                    .into(),
            );
        }
        let scene_bounds = if interactive {
            Some(
                conduitos::native_face_scene::NativeFaceScene::visual_fallback_memory_bounds(face)
                    .map_err(|error| format!("native interactive Scene qualification:{error:?}"))?,
            )
        } else {
            None
        };
        let format = output.format();
        if format.width != u32::from(self.width) || format.height != u32::from(self.height) {
            return Err("native Mask actual output geometry changed".into());
        }
        if !interactive {
            face.validate()
                .map_err(|error| format!("native Face validation: {error:?}"))?;
        }
        if front_subject.is_empty()
            || front_subject.len() > conduit_face::MAX_MANIFESTATION_TARGET_BYTES
        {
            return Err("native Mask front subject exceeds finite profile".into());
        }
        let plan_bytes = native_mask_plan_backing_bytes(&self.plan)
            .ok_or("unsupported selected Mask closure")?;
        // Reserve only the remaining authored capacity. This is cooperative
        // preparation accounting, not physical allocator/RSS containment.
        let mut preparation = reserve_preparation(host, provider)?;
        let face_bytes = face
            .retained_backing_bytes()
            .ok_or("native Mask finite Face backing overflow")?;
        let input_bytes = super::outer_bound::input_owner_bytes(plan_bytes, face_bytes)?;
        if input_bytes > preparation.bytes() as usize {
            host.kernel_resources
                .release_base_backing(preparation)
                .map_err(|(_, error)| error)?;
            return Err("native Mask input owners exceed authored preparation ceiling".into());
        }
        let mut interaction_heap = if interaction_bytes.is_some() {
            let bytes = super::outer_bound::checked_invocation_bytes(
                &self.plan,
                face,
                front_subject,
                &self.base,
            )?;
            let scene_peak = scene_bounds
                .as_ref()
                .and_then(|bounds| bounds.lifetime_peak_bytes())
                .ok_or("native input Scene lifetime peak overflow")?;
            let minimum = input_bytes
                .checked_add(scene_peak)
                .ok_or("native input preparation sum overflow")?;
            let available = preparation.bytes() as usize;
            if bytes < minimum || bytes > available {
                host.kernel_resources
                    .release_base_backing(preparation)
                    .map_err(|(_, error)| error)?;
                return Err(format!("native input lifetime envelope refused: requested {bytes}, minimum input owners {minimum}, authored remaining pool {available}"));
            }
            host.kernel_resources
                .retain_base_preparation_bytes(&mut preparation, bytes)?;
            Some(host.kernel_resources.reserve_managed_base_memory(bytes)?)
        } else {
            None
        };
        let before = self
            .compositor
            .ordinary_retained_backing_bytes()
            .ok_or("unsupported native compositor owner")?;
        let result = (|| {
            // Interactive validation can inspect typed/content contracts; its
            // temporary owners belong under the pre-reserved invocation lease.
            if interactive {
                face.validate()
                    .map_err(|error| format!("native Face validation: {error:?}"))?;
            }
            // Construct the preference only under the original invocation lease.
            let expression = if interactive {
                Some(super::outer_bound::interactive_expression()?)
            } else {
                None
            };
            let scene = PreparedSelectedScene::prepare(
                face.clone(),
                self.width,
                self.height,
                interactive,
                expression.as_ref(),
            )
            .map_err(|error| format!("native semantic scene: {error:?}"))?;
            let frame = scene
                .frame()
                .map_err(|error| format!("native semantic frame: {error:?}"))?;
            let storage = self
                .storage
                .take()
                .ok_or("native Mask stores unavailable")?;
            self.failed = true;
            let cold = PreparedNativeMaskPlay::prepare_cold_with_storage(
                &self.plan,
                face,
                play_sequence,
                front_subject,
                SURFACE,
                self.base.clone(),
                NativeMaskStorageProfile::Standard,
                storage,
            )
            .map_err(|error| format!("native Mask cold preparation: {error:?}"))?;
            let request = cold
                .renderer_request()
                .map_err(|error| format!("native Mask request: {error:?}"))?;
            let mut possession = ActiveNativeSurfacePossession::begin(
                provider,
                &self.plan.plan,
                request.prepared_show().show.active_play_id.clone(),
            )
            .map_err(|error| {
                format!("native Mask exact resource-Cord possession refused: {error:?}")
            })?;
            possession.authorize().map_err(|error| {
                format!("native Mask actual resource-Cord authority refused: {error:?}")
            })?;
            // The same admitted metrics govern hit targets and painted text.
            let composition = if let Some(expression) = expression.as_ref() {
                self.compositor.update_pending_configured_mask_surface(
                    request.face(),
                    request.prepared_show(),
                    request.display_base_id(),
                    &frame.scene,
                    expression,
                )
            } else {
                self.compositor.update_pending_mask_surface(
                    request.face(),
                    request.prepared_show(),
                    request.display_base_id(),
                    &frame.scene,
                )
            }
            .map_err(|error| format!("native Mask offscreen raster: {error:?}"))?
            .clone();
            let bytes = cold
                .retained_metadata_bytes()
                .and_then(|bytes| bytes.checked_add(possession.retained_backing_bytes()?))
                .and_then(|bytes| bytes.checked_add(scene.retained_backing_bytes()?))
                .and_then(|bytes| bytes.checked_add(frame.retained_backing_bytes()?))
                .and_then(|bytes| bytes.checked_add(composition.ordinary_retained_backing_bytes()?))
                .and_then(|bytes| {
                    bytes.checked_add(
                        self.compositor
                            .ordinary_retained_backing_bytes()?
                            .checked_sub(before)?,
                    )
                })
                .and_then(|bytes| bytes.checked_add(ImageMemoryLease::owned_backing_bytes()))
                .ok_or("native Mask actual per-frame backing unsupported or overflowing")?;
            let heap = if let Some(heap) = interaction_heap.take() {
                let transient = scene_bounds
                    .as_ref()
                    .ok_or("interactive Scene bound absent")?
                    .input_transient_bytes;
                let actual_peak = bytes
                    .checked_add(transient)
                    .ok_or("native observed input peak overflow")?;
                if actual_peak > preparation.bytes() as usize {
                    return Err(
                        "native actual cold/input owners exceed checked invocation bound".into(),
                    );
                }
                heap
            } else {
                host.kernel_resources
                    .retain_base_preparation_bytes(&mut preparation, bytes)?;
                host.kernel_resources.reserve_managed_base_memory(bytes)?
            };
            drop(frame);
            // Recheck the actual authenticated encounter after preparation.
            validate_encounter(encounter, route, &host.advertisement, face)?;
            // No kernel Step or physical effect precedes the actual owner lease.
            let pending = cold
                .start()
                .map_err(|error| format!("native Mask activation: {error:?}"))?;
            validate_encounter(encounter, route, &host.advertisement, face)?;
            self.compositor
                .compose_frame(output)
                .map_err(|error| format!("native Mask compose: {error:?}"))?;
            validate_encounter(encounter, route, &host.advertisement, face)?;
            output
                .commit_frame()
                .map_err(|error| format!("native Mask physical commit: {error:?}"))?;
            let ack = self
                .compositor
                .scanout_acknowledgement(&composition, face.revision)
                .ok_or("native Mask physical acknowledgment absent")?;
            let session = pending
                .complete_render(&ack)
                .map_err(|error| format!("native Mask renderer completion: {error:?}"))?;
            if interactive {
                // As in the shared NativeFaceMask, focus only an actually
                // acknowledged surface; prepared pixels cannot receive input.
                self.compositor
                    .focus_surface(SURFACE)
                    .map_err(|error| format!("native input focus:{error:?}"))?;
                self.compositor
                    .compose_frame(output)
                    .map_err(|error| format!("native focus compose:{error:?}"))?;
                validate_encounter(encounter, route, &host.advertisement, face)?;
                output
                    .commit_frame()
                    .map_err(|error| format!("native focus commit:{error:?}"))?;
                if self
                    .compositor
                    .scanout_acknowledgement(&composition, face.revision)
                    .is_none()
                {
                    return Err("native focus lacks physical acknowledgement".into());
                }
            }
            Ok((
                session,
                composition,
                possession,
                heap,
                scene.into_interactive(),
            ))
        })();
        match result {
            Ok((session, composition, possession, heap, scene)) => {
                let frame = SelectedNativeMaskFrame {
                    face_revision: face.revision,
                    play_sequence,
                    scanout_frame_sequence: self.compositor.frame_sequence(),
                };
                self.session = Some(session);
                self.scene = scene;
                self.frame = Some(ActiveFrameCustody {
                    composition,
                    possession,
                    _heap: heap,
                    pool: preparation,
                });
                self.last_play = Some(play_sequence);
                self.failed = false;
                Ok(frame)
            }
            Err(error) => {
                // Cold/session/Scene owners have dropped; offscreen compositor
                // metadata remains. Keep its uncertain pool custody poisoned.
                self.failed = true;
                Err(error)
            }
        }
    }
    pub(super) fn take_evidence(&mut self) -> Result<Option<SelectedNativeMaskEvidence>, String> {
        let Some(session) = self.session.take() else {
            return Ok(None);
        };
        match session.retire_retained() {
            Ok((storage, evidence)) => self.complete_frame(storage, evidence).map(Some),
            Err(error) => {
                self.failed = true;
                Err(format!("native Mask evidence handoff: {error:?}"))
            }
        }
    }
}
pub(super) fn reserve_preparation(
    host: &mut StdHost,
    provider: &NativeSurfaceProvider,
) -> Result<BaseBackingReservation, String> {
    let pool = provider
        .entry
        .resources
        .iter()
        .find(|pool| pool.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS)
        .ok_or("native Mask runtime pool absent")?;
    host.kernel_resources.reserve_base_preparation_ceiling(
        &host.advertisement,
        &host.base_registry,
        &provider.entry.provider_instance_id,
        provider.entry.provider_generation,
        &pool.pool_id,
    )
}
