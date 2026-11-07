//! Checked selected read-only Mask: actual stores and driver custody, no local
//! forwarding Plot, fabricated publication, membership or Show acknowledgment.
#[cfg(test)]
mod encounter_fixture;
mod initialization;
mod render;
mod retirement;
#[cfg(test)]
mod tests;
use crate::{
    kernel_preparation::{
        BaseBackingReservation, BodyMemoryReceipt, ImageMemoryLease, KernelResourceReservation,
    },
    StdHost,
};
use conduit_conduitos_mask_offer::{NativeMaskStorageProfile, READ_ONLY_INTERACTION_PROFILE};
use conduit_core::*;
use conduit_face::{Face, PlannedMaskPlot, RemoteOwnerMaskRouteSeal};
use conduitos::{
    native_compositor::{CompositionReceipt, PreparedCompositorBacking},
    product_bases::NativeSurfaceProvider,
    product_bases::{
        NativeMaskBackingReceipt, RetainedNativeMaskStorage, RetiredNativeMaskEvidence,
        UnboundNativeMaskStorage,
    },
};
pub(crate) use initialization::InitialNativeMask;
use render::{BoundReadOnlyMask, PreparedReadOnlyBinding};
pub use render::{SelectedNativeMaskEvidence, SelectedNativeMaskFrame};

pub(crate) const SURFACE: &str = "selected/native-mask";

/// Trusted cooperative hosted carrier adapter. Implementations must retain and
/// recheck authenticated membership, pinned peer, exact Face/route/Host binding
/// and their current finite lifetime. Received wire data alone is insufficient.
/// This is an explicit hosted TCB seam, not a hostile-code capability boundary.
pub trait NativeMaskEncounter {
    fn validate_current(
        &self,
        face: &Face,
        route: &RemoteOwnerMaskRouteSeal,
        local_offer: &HostAdvertisement,
    ) -> Result<(), String>;
}
fn validate_encounter(
    encounter: &impl NativeMaskEncounter,
    route: &RemoteOwnerMaskRouteSeal,
    local: &HostAdvertisement,
    face: &Face,
) -> Result<(), String> {
    route
        .validate_mask_host_offer(local)
        .map_err(|error| format!("selected native local route: {error:?}"))?;
    if route.face_id != face.identity
        || route.face_revision != face.revision
        || route.face_basis != face.basis
    {
        return Err("selected native Face differs from exact owner route".into());
    }
    encounter.validate_current(face, route, local)
}

/// Fixed managed kernel/value/Sign/raster and actual scanout CPU stores only.
/// Source/registry metadata, SDL/GPU allocation, stack, allocator overhead and
/// whole preparation peak/RSS are separate and are not proven by this receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeMaskFixedBackingReceipt {
    pub mask: NativeMaskBackingReceipt,
    pub compositor_bytes: usize,
    pub scanout_cpu_bytes: usize,
    pub runtime_pool_bytes: usize,
    pub image_heap_charge: usize,
}
// The finite Sign arrays are inline in the unbound owner. Boxing this enum would
// add another allocation during binding; no whole-stack/Root-peak claim is made.
#[allow(clippy::large_enum_variant)]
enum Stores {
    Unbound {
        kernel: UnboundNativeMaskStorage,
        raster: PreparedCompositorBacking,
    },
    Bound {
        renderer: BoundReadOnlyMask,
        _plan_custody: Vec<KernelResourceReservation>,
        _metadata_pool: BaseBackingReservation,
        _metadata_heap: ImageMemoryLease,
        _kernel_metadata_heap: ImageMemoryLease,
    },
    Lost {
        _plan_custody: Vec<KernelResourceReservation>,
        _metadata_pool: Option<BaseBackingReservation>,
    },
}
/// Retained actual driver-provider key and fixed stores. This prepares a Back;
/// only successful current-route admission and physical commit produce a Show.
pub struct SelectedNativeMaskBase {
    stores: Stores,
    provider: NativeSurfaceProvider,
    backing: BaseBackingReservation,
    _heap: ImageMemoryLease,
    receipt: NativeMaskFixedBackingReceipt,
}
impl SelectedNativeMaskBase {
    pub fn fixed_backing(&self) -> NativeMaskFixedBackingReceipt {
        self.receipt
    }
    pub fn entry(&self) -> &BaseProviderEntry {
        &self.provider.entry
    }
    pub fn plan(&self) -> Option<&PlannedMaskPlot> {
        match &self.stores {
            Stores::Bound { renderer, .. } => Some(renderer.plan()),
            _ => None,
        }
    }
    pub(crate) fn healthy(&self) -> bool {
        match &self.stores {
            Stores::Unbound { .. } => true,
            Stores::Bound { renderer, .. } => renderer.healthy(),
            Stores::Lost { .. } => false,
        }
    }
    pub(crate) fn required_bytes(width: u32, height: u32) -> Result<(usize, usize), String> {
        let kernel =
            RetainedNativeMaskStorage::required_store_bytes(NativeMaskStorageProfile::Standard)
                .map_err(|error| format!("selected Mask store: {error:?}"))?;
        let raster = PreparedCompositorBacking::required_bytes(
            u16::try_from(width).map_err(|_| "Mask width")?,
            u16::try_from(height).map_err(|_| "Mask height")?,
        )
        .map_err(|error| format!("selected raster store: {error:?}"))?;
        Ok((kernel, raster))
    }
    pub(crate) fn initialize(
        host: &mut StdHost,
        provider: NativeSurfaceProvider,
        width: u32,
        height: u32,
        scanout_cpu_bytes: usize,
    ) -> Result<Self, String> {
        let (kernel_bytes, raster_bytes) = Self::required_bytes(width, height)?;
        let runtime_pool_bytes = kernel_bytes
            .checked_add(raster_bytes)
            .ok_or("selected Mask store sum")?;
        let image_heap_charge = runtime_pool_bytes
            .checked_add(scanout_cpu_bytes)
            .and_then(|bytes| bytes.checked_add(ImageMemoryLease::owned_backing_bytes()))
            .ok_or("selected Mask heap sum")?;
        let heap = host
            .kernel_resources
            .reserve_managed_base_memory(image_heap_charge)?;
        let pool = provider
            .entry
            .resources
            .iter()
            .find(|resource| resource.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS)
            .ok_or("selected Mask memory pool absent")?;
        let backing = host.kernel_resources.reserve_base_backing(
            &host.advertisement,
            &host.base_registry,
            &provider.entry.provider_instance_id,
            provider.entry.provider_generation,
            &pool.pool_id,
            u32::try_from(runtime_pool_bytes)
                .map_err(|_| "selected Mask backing exceeds pool units")?,
        )?;
        let kernel =
            UnboundNativeMaskStorage::prepare(NativeMaskStorageProfile::Standard, kernel_bytes)
                .map_err(|error| format!("selected Mask allocation: {error:?}"))?;
        let mask = kernel.backing();
        let raster = PreparedCompositorBacking::prepare(
            SURFACE,
            u16::try_from(width).map_err(|_| "Mask width")?,
            u16::try_from(height).map_err(|_| "Mask height")?,
            raster_bytes,
        )
        .map_err(|error| format!("selected raster allocation: {error:?}"))?;
        if mask.managed_store_bytes != kernel_bytes
            || raster.backing_bytes() != raster_bytes
            || backing.bytes() as usize != runtime_pool_bytes
        {
            return Err("actual selected stores differ from reserved fixed backing".into());
        }
        Ok(Self {
            stores: Stores::Unbound { kernel, raster },
            provider,
            backing,
            _heap: heap,
            receipt: NativeMaskFixedBackingReceipt {
                mask,
                compositor_bytes: raster_bytes,
                scanout_cpu_bytes,
                runtime_pool_bytes,
                image_heap_charge,
            },
        })
    }
    fn validate_binding(&self, host: &StdHost) -> Result<(), String> {
        if !self.healthy()
            || !self.backing.matches_ledger(&host.kernel_resources)
            || !host.base_registry.entries().contains(&self.provider.entry)
            || !host
                .advertisement
                .bases
                .iter()
                .any(|base| crate::hosted_display::descriptor_matches(base, &self.provider.entry))
            || self.provider.entry.capabilities.iter().any(|capability| {
                host.advertisement
                    .capabilities
                    .iter()
                    .filter(|offered| offered.capability_id == capability.capability_id)
                    .ne(std::iter::once(capability))
            })
            || self.provider.entry.resources.iter().any(|pool| {
                host.advertisement
                    .resources
                    .iter()
                    .filter(|offered| offered.pool_id == pool.pool_id)
                    .ne(std::iter::once(pool))
            })
        {
            return Err(
                "selected native Mask provider is lost or differs from final Host registry".into(),
            );
        }
        Ok(())
    }
    fn bind(&mut self, host: &mut StdHost, planned: &PlannedMaskPlot) -> Result<(), String> {
        if let Stores::Bound { renderer, .. } = &self.stores {
            if renderer.plan() == planned {
                return Ok(());
            }
        }
        let mut inputs = planned
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|gear| gear.kind_id.as_str() == conduit_face::FACE_INTERACTION_KIND);
        let input = inputs
            .next()
            .ok_or("selected Mask interaction Back absent")?;
        if inputs.next().is_some()
            || input.execution_profile_id.as_str() != READ_ONLY_INTERACTION_PROFILE
            || !input.resources.is_empty()
            || !input.authority.is_empty()
        {
            return Err("selected Mask requires its narrowed read-only interaction Back".into());
        }
        let input_plan_bytes = conduitos::product_bases::native_mask_plan_backing_bytes(planned)
            .ok_or("selected Mask Plan owns unsupported metadata")?;
        if matches!(&self.stores, Stores::Bound { .. }) {
            self.retire_bound_plan(host)?;
        }
        let mut preparation = render::reserve_preparation(host, &self.provider)?;
        if input_plan_bytes > preparation.bytes() as usize {
            host.kernel_resources
                .release_base_backing(preparation)
                .map_err(|(_, error)| error)?;
            return Err("selected Mask input Plan exceeds authored preparation ceiling".into());
        }
        let previous = core::mem::replace(
            &mut self.stores,
            Stores::Lost {
                _plan_custody: Vec::new(),
                _metadata_pool: None,
            },
        );
        let (kernel, raster) = match previous {
            Stores::Unbound { kernel, raster } => (kernel, raster),
            other => {
                self.stores = other;
                return Err("selected Mask is not unbound".into());
            }
        };
        let prepared = PreparedReadOnlyBinding::prepare(planned.clone(), &self.provider, raster)?;
        let metadata_bytes = prepared
            .metadata_bytes(self.receipt.compositor_bytes)?
            .checked_add(ImageMemoryLease::owned_backing_bytes())
            .ok_or("selected Mask binding heap overflow")?;
        if metadata_bytes > preparation.bytes() as usize {
            return Err("selected Mask binding exceeds authored preparation ceiling".into());
        }
        // This receipt is computed from the actual prepared Plan/compositor
        // owners. Existing IMAGE guard remains in force before kernel binding.
        let receipt = BodyMemoryReceipt::prepared_native_mask(metadata_bytes);
        let (reservations, heap) = host.kernel_resources.reserve_body_memory(
            &host.advertisement,
            &[(&prepared.plan().plan, false)],
            &receipt,
        )?;
        let result = prepared.bind(kernel).and_then(|renderer| {
            let kernel_bytes = renderer
                .kernel_metadata_bytes()?
                .checked_add(ImageMemoryLease::owned_backing_bytes())
                .ok_or("selected Mask kernel metadata overflow")?;
            let total = metadata_bytes
                .checked_add(kernel_bytes)
                .ok_or("selected Mask metadata sum overflow")?;
            host.kernel_resources
                .retain_base_preparation_bytes(&mut preparation, total)?;
            let kernel_heap = host
                .kernel_resources
                .reserve_managed_base_memory(kernel_bytes)?;
            Ok((renderer, kernel_heap))
        });
        match result {
            Ok((renderer, kernel_heap)) => {
                self.stores = Stores::Bound {
                    renderer,
                    _plan_custody: reservations,
                    _metadata_pool: preparation,
                    _metadata_heap: heap,
                    _kernel_metadata_heap: kernel_heap,
                };
                Ok(())
            }
            Err(error) => {
                for reservation in reservations {
                    host.kernel_resources.release(reservation)?;
                }
                Err(error)
            }
        }
    }
}
impl StdHost {
    pub fn ready_selected_native_mask(&self) -> Option<&SelectedNativeMaskBase> {
        let scanout = self.ready_selected_scanout()?;
        scanout
            .native_mask
            .as_ref()
            .filter(|mask| mask.validate_binding(self).is_ok())
    }
    /// Borrow only the actual acknowledged Show while its child Play and
    /// selected physical provider remain current. No reconstructed receipt.
    pub fn selected_native_mask_show(&self) -> Option<&conduit_face::MaskShow> {
        match &self.ready_selected_native_mask()?.stores {
            Stores::Bound { renderer, .. } => renderer.show(),
            _ => None,
        }
    }
    /// Consume an already authorized owner Face through the real selected Mask
    /// Fore. No local snapshot-forwarding Plot or publication receipt is made.
    pub fn render_selected_native_mask(
        &mut self,
        encounter: &impl NativeMaskEncounter,
        route: &RemoteOwnerMaskRouteSeal,
        face: &Face,
        play_sequence: u64,
        front_subject: &str,
        output: &mut conduit_linux_display::Scanout<'_>,
    ) -> Result<SelectedNativeMaskFrame, String> {
        let scanout = self
            .ready_selected_scanout()
            .ok_or("selected physical output is lost")?;
        if !output.matches_initialized(scanout.initialized()) {
            return Err("render output differs from selected physical claim".into());
        }
        validate_encounter(encounter, route, &self.advertisement, face)?;
        let mut mask = self
            .selected_scanout
            .as_mut()
            .and_then(|scanout| scanout.native_mask.take())
            .ok_or("selected native Mask is absent")?;
        let result = (|| {
            mask.validate_binding(self)?;
            mask.bind(self, &route.planned_mask)?;
            match &mut mask.stores {
                Stores::Bound { renderer, .. } => renderer.render(
                    self,
                    &mask.provider,
                    encounter,
                    route,
                    face,
                    play_sequence,
                    front_subject,
                    output,
                ),
                _ => Err("selected Mask binding failed".into()),
            }
        })();
        if matches!(&mask.stores, Stores::Bound {renderer, ..} if !renderer.healthy()) {
            let previous = core::mem::replace(
                &mut mask.stores,
                Stores::Lost {
                    _plan_custody: Vec::new(),
                    _metadata_pool: None,
                },
            );
            if let Stores::Bound {
                _plan_custody,
                _metadata_pool,
                ..
            } = previous
            {
                mask.stores = Stores::Lost {
                    _plan_custody,
                    _metadata_pool: Some(_metadata_pool),
                };
            }
        }
        if let Some(scanout) = self.selected_scanout.as_mut() {
            scanout.native_mask = Some(mask);
        }
        result
    }
    /// Transfers complete local evidence custody, never a durable recording ACK.
    pub fn take_selected_native_mask_evidence(
        &mut self,
    ) -> Result<Option<SelectedNativeMaskEvidence>, String> {
        let scanout = self
            .selected_scanout
            .as_mut()
            .ok_or("selected scanout absent")?;
        let mask = scanout.native_mask.as_mut().ok_or("selected Mask absent")?;
        match &mut mask.stores {
            Stores::Bound { renderer, .. } => renderer.take_evidence(),
            _ => Ok(None),
        }
    }
    /// Explicit local disposal after evidence custody was consumed. This is not
    /// a durable history acknowledgment and cannot authorize another member.
    pub fn retire_selected_native_mask_evidence(
        &mut self,
        evidence: &mut Option<SelectedNativeMaskEvidence>,
    ) -> Result<(), String> {
        let owner = evidence
            .as_ref()
            .ok_or("native Mask evidence custody absent")?;
        if !owner.matches_host(self) {
            return Err(
                "native Mask evidence belongs to another Host/Boot/ledger; custody retained".into(),
            );
        }
        evidence
            .take()
            .ok_or("native Mask evidence custody absent")?
            .retire(&mut self.kernel_resources)
    }
}
