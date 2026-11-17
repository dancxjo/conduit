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
mod invocation;
use interactive::PreparedSelectedScene;
pub use interactive::{SelectedNativeInteraction, SelectedNativeMaskInput};

pub(super) struct PreparedReadOnlyBinding {
    plan: PlannedMaskPlot,
    compositor: NativeCompositor,
    base: HostBaseId,
    width: u16,
    height: u16,
    images: Option<std::sync::Arc<images::NativeMaskImages>>,
}
impl PreparedReadOnlyBinding {
    pub(super) fn prepare(
        plan: PlannedMaskPlot,
        provider: &NativeSurfaceProvider,
        raster: PreparedCompositorBacking,
        images: Option<std::sync::Arc<images::NativeMaskImages>>,
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
            images,
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
            images: self.images,
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
    images: Option<std::sync::Arc<images::NativeMaskImages>>,
    pub(super) scene: Option<conduitos::native_face_scene::NativeFaceScene>,
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
