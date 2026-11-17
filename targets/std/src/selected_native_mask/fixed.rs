//! Prepare actual unbound kernel/raster owners only after original-counter funding.
use super::*;
impl SelectedNativeMaskBase {
    pub(crate) fn required_bytes(width: u32, height: u32) -> Result<(usize, usize), String> {
        let kernel =
            RetainedNativeMaskStorage::required_store_bytes(NativeMaskStorageProfile::Standard)
                .map_err(|error| format!("selected Mask store:{error:?}"))?;
        let raster = PreparedCompositorBacking::required_bytes(
            u16::try_from(width).map_err(|_| "Mask width")?,
            u16::try_from(height).map_err(|_| "Mask height")?,
        )
        .map_err(|error| format!("selected raster store:{error:?}"))?;
        Ok((kernel, raster))
    }
    pub(crate) fn initialize(
        host: &mut StdHost,
        initial: super::initialization::InitialNativeMask,
    ) -> Result<Self, String> {
        let super::initialization::InitialNativeMask {
            provider,
            entry,
            width,
            height,
            scanout_cpu_bytes,
            input,
            images,
            heap,
            image_heap_charge,
            runtime_pool_bytes,
            token_bytes,
            ..
        } = initial;
        host.kernel_resources.require_input_memory_owner(&heap)?;
        let (kernel_bytes, raster_bytes) = Self::required_bytes(width, height)?;
        let pool = entry
            .resources
            .iter()
            .find(|pool| pool.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS)
            .ok_or("selected Mask memory pool absent")?;
        let backing = host.kernel_resources.reserve_base_backing(
            &host.advertisement,
            &host.base_registry,
            &entry.provider_instance_id,
            entry.provider_generation,
            &pool.pool_id,
            u32::try_from(runtime_pool_bytes).map_err(|_| "selected native pool exceeds units")?,
        )?;
        if backing
            .owned_identity_bytes()
            .is_none_or(|actual| actual > token_bytes)
        {
            return Err("native reservation identity exceeds preflight".into());
        }
        let kernel =
            UnboundNativeMaskStorage::prepare(NativeMaskStorageProfile::Standard, kernel_bytes)
                .map_err(|error| format!("selected Mask allocation:{error:?}"))?;
        let mask = kernel.backing();
        let raster = PreparedCompositorBacking::prepare(
            SURFACE,
            u16::try_from(width).map_err(|_| "Mask width")?,
            u16::try_from(height).map_err(|_| "Mask height")?,
            raster_bytes,
        )
        .map_err(|error| format!("selected raster allocation:{error:?}"))?;
        if mask.managed_store_bytes != kernel_bytes
            || raster.backing_bytes() != raster_bytes
            || backing.bytes() as usize != runtime_pool_bytes
        {
            return Err("actual selected stores differ from reserved fixed backing".into());
        }
        let images = images
            .map(|images| images::NativeMaskImages::initialize(host, &entry, images))
            .transpose()?;
        Ok(Self {
            _kernel: kernel,
            _raster: raster,
            input,
            images,
            _provider: provider,
            entry,
            _backing: backing,
            receipt: NativeMaskFixedBackingReceipt {
                mask,
                compositor_bytes: raster_bytes,
                scanout_cpu_bytes,
                runtime_pool_bytes,
                image_heap_charge,
            },
            _heap: heap,
        })
    }
}
