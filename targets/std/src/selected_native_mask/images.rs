//! Explicitly owned immutable pixels; a key or received Face grants no access.
use super::*;
use conduit_body::BodyId;
use sha2::{Digest, Sha256};
pub const NATIVE_MASK_IMAGE_PREPARATION_BYTES: usize = 12 * 1024 * 1024;
pub const NATIVE_MASK_IMAGE_RETAINED_BYTES: usize = 10 * 1024 * 1024;
pub struct NativeMaskImagePlane {
    pub original_sha256: [u8; 32],
    pub version_sha256: [u8; 32],
    pub width: u16,
    pub height: u16,
    pub rgba: Box<[u8]>,
}
pub struct NativeMaskImageData {
    planes: [NativeMaskImagePlane; 4],
}
impl NativeMaskImageData {
    /// Prepared opaque data only: this constructs neither a resolver nor possession.
    pub fn from_owned_planes(planes: [NativeMaskImagePlane; 4]) -> Result<Self, String> {
        for (index, plane) in planes.iter().enumerate() {
            let pixels = usize::from(plane.width) * usize::from(plane.height);
            if plane.width == 0
                || plane.height == 0
                || plane.width > 1024
                || plane.height > 600
                || pixels > 1024 * 600
                || plane.rgba.len() != pixels * 4
                || plane
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel[3] != 255)
                || planes[..index]
                    .iter()
                    .any(|prior| prior.original_sha256 == plane.original_sha256)
            {
                return Err("native image exact opaque plane bounds differ".into());
            }
            let mut hash = Sha256::new();
            hash.update(b"conduit-ambient-rgba-v1\0");
            hash.update(plane.original_sha256);
            hash.update(u32::from(plane.width).to_le_bytes());
            hash.update(u32::from(plane.height).to_le_bytes());
            hash.update(&plane.rgba);
            let actual: [u8; 32] = hash.finalize().into();
            if actual != plane.version_sha256 {
                return Err("native image exact version differs".into());
            }
        }
        Ok(Self { planes })
    }
    fn bytes(&self) -> usize {
        self.planes.iter().map(|plane| plane.rgba.len()).sum()
    }
}
pub struct NativeMaskImageInitialization {
    body: BodyId,
    pub(super) preparation_bytes: usize,
    retained_bytes: usize,
    factory: Box<dyn FnOnce() -> Result<NativeMaskImageData, String>>,
}
impl NativeMaskImageInitialization {
    /// # Safety
    /// Root must independently authorize this Body's immutable assets and ALL
    /// factory effects/backing. Bounds include captures, data, metadata and lease
    /// storage; the factory must not retain side owners or exceed that peak.
    pub unsafe fn root_authorized(
        body: BodyId,
        preparation_bytes: usize,
        retained_bytes: usize,
        factory: impl FnOnce() -> Result<NativeMaskImageData, String> + 'static,
    ) -> Result<Self, String> {
        if body.as_str().len() > 256
            || retained_bytes == 0
            || preparation_bytes < retained_bytes
            || preparation_bytes > NATIVE_MASK_IMAGE_PREPARATION_BYTES
            || retained_bytes > NATIVE_MASK_IMAGE_RETAINED_BYTES
        {
            return Err("native image factory has invalid finite bounds".into());
        }
        Ok(Self {
            body,
            preparation_bytes,
            retained_bytes,
            factory: Box::new(factory),
        })
    }
}
pub(super) struct FundedImages {
    pub initial: NativeMaskImageInitialization,
    pub heap: ImageMemoryLease,
    pub token_bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeMaskImageBackingReceipt {
    pub retained_data_bytes: usize,
    pub runtime_pool_bytes: usize,
    pub image_heap_charge: usize,
}
pub(super) struct NativeMaskImages {
    _data: NativeMaskImageData,
    _body: BodyId,
    _reservation: BaseBackingReservation,
    receipt: NativeMaskImageBackingReceipt,
    _heap: ImageMemoryLease,
}
impl NativeMaskImages {
    pub(super) fn initialize(
        host: &mut StdHost,
        entry: &BaseProviderEntry,
        funded: FundedImages,
    ) -> Result<Self, String> {
        let FundedImages {
            initial,
            heap,
            token_bytes,
        } = funded;
        host.kernel_resources.require_input_memory_owner(&heap)?;
        let pool = entry
            .resources
            .iter()
            .find(|pool| pool.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS)
            .ok_or("native images pool absent")?;
        let reservation = host.kernel_resources.reserve_base_backing(
            &host.advertisement,
            &host.base_registry,
            &entry.provider_instance_id,
            entry.provider_generation,
            &pool.pool_id,
            u32::try_from(
                initial
                    .preparation_bytes
                    .checked_add(token_bytes)
                    .ok_or("native image reservation overflow")?,
            )
            .map_err(|_| "native images preparation units")?,
        )?;
        let actual_token = reservation
            .owned_identity_bytes()
            .ok_or("native image identity overflow")?;
        if actual_token > token_bytes {
            return Err("native image reservation identity exceeds preflight".into());
        }
        let data = (initial.factory)()?;
        let retained = data
            .bytes()
            .checked_add(initial.body.retained_backing_bytes())
            .and_then(|bytes| bytes.checked_add(ImageMemoryLease::owned_backing_bytes()))
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<Self>()))
            .ok_or("native images retained sum")?;
        if retained > initial.retained_bytes {
            return Err("native images exceed admitted retained data".into());
        }
        let retained = retained
            .checked_add(actual_token)
            .ok_or("native image retained identity overflow")?;
        Ok(Self {
            _data: data,
            _body: initial.body,
            _reservation: reservation,
            receipt: NativeMaskImageBackingReceipt {
                retained_data_bytes: retained,
                runtime_pool_bytes: initial.preparation_bytes + token_bytes,
                image_heap_charge: heap.reserved_bytes(),
            },
            _heap: heap,
        })
    }
    pub(super) fn receipt(&self) -> NativeMaskImageBackingReceipt {
        self.receipt
    }
}
