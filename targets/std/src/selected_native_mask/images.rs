//! Private renderer implementation assets. Keys and received Face data grant
//! no access; only a current authorized native Mask possession opens views.
use super::*;
use conduit_body::BodyId;
use conduit_face::GraphicsImageKey;
use conduitos::{
    display::{DisplayError, ImageResolver, OpaqueRgbaImage},
    product_bases::ActiveNativeSurfacePossession,
};
mod cache_export;
#[cfg(test)]
use crate::ambient_wallpaper::cache::MAXIMUM_MANIFEST_BYTES;
use crate::ambient_wallpaper::cache::version;
#[cfg(test)]
use sha2::{Digest, Sha256};
use std::path::Path;

const MAXIMUM_PIXELS: usize = 1024 * 600;
pub const NATIVE_MASK_IMAGE_PREPARATION_BYTES: usize = 12 * 1024 * 1024;
pub const NATIVE_MASK_IMAGE_RETAINED_BYTES: usize = 10 * 1024 * 1024;

pub struct NativeMaskImageInitialization {
    body: BodyId,
    pub(super) preparation_bytes: usize,
    retained_bytes: usize,
    factory: Box<dyn FnOnce() -> Result<NativeMaskImageData, String>>,
}
impl NativeMaskImageInitialization {
    /// # Safety
    /// Root must independently authorize this Body's immutable renderer assets,
    /// the factory's filesystem effects and complete finite preparation work.
    /// Budgets include captures, owned data, metadata and lease backing; factory
    /// construction must not perform acquisition or claim received Face authority.
    /// The factory must neither exceed its declared peak nor retain side owners.
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
struct Photo {
    key: GraphicsImageKey,
    width: u16,
    height: u16,
    rgba: Box<[u8]>,
}
/// Prepared data only, with no resolver or possession constructor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeMaskImageBackingReceipt {
    pub retained_data_bytes: usize,
    pub runtime_pool_bytes: usize,
    pub image_heap_charge: usize,
}

pub struct NativeMaskImageData {
    photos: [Photo; 4],
}
fn photo(
    original: [u8; 32],
    expected: [u8; 32],
    width: u16,
    height: u16,
    rgba: Box<[u8]>,
) -> Result<Photo, String> {
    let count = usize::from(width) * usize::from(height);
    if width > 1024
        || height > 600
        || count == 0
        || count > MAXIMUM_PIXELS
        || rgba.len() != count * 4
        || rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255)
        || version(&original, width, height, &rgba) != expected
    {
        return Err("native image pixels or exact version differ".into());
    }
    let key = GraphicsImageKey::new(original, expected)
        .map_err(|_| "native image identity is invalid")?;
    Ok(Photo {
        key,
        width,
        height,
        rgba,
    })
}
impl NativeMaskImageData {
    pub fn from_prepared_deck(
        deck: &crate::ambient_wallpaper::acquisition::PreparedDeck,
    ) -> Result<Self, String> {
        let mut photos = Vec::with_capacity(4);
        for p in deck.photos() {
            photos.push(photo(
                *p.original_sha256(),
                *p.version_sha256(),
                u16::try_from(p.width()).map_err(|_| "image width")?,
                u16::try_from(p.height()).map_err(|_| "image height")?,
                p.rgba().into(),
            )?);
        }
        Self::finish(photos)
    }
    fn finish(photos: Vec<Photo>) -> Result<Self, String> {
        for (index, p) in photos.iter().enumerate() {
            if photos[..index]
                .iter()
                .any(|other| other.key.identity() == p.key.identity())
            {
                return Err("native image deck repeats original content".into());
            }
        }
        Ok(Self {
            photos: photos
                .try_into()
                .map_err(|_| "native image deck requires four generations")?,
        })
    }
    fn owned_bytes(&self) -> usize {
        self.photos.iter().map(|p| p.rgba.len()).sum::<usize>()
    }
    /// Load only preprepared opaque RGBA. No decoder, network, retry or fallback.
    /// Root owns authorization of the manifest path and its immutable directory.
    /// Files are opened relative to one held directory FD with O_NOFOLLOW.
    pub fn load_cache(path: &Path, expected_manifest_sha256: [u8; 32]) -> Result<Self, String> {
        let cached = crate::ambient_wallpaper::cache::load_photos(path, expected_manifest_sha256)?;
        let mut photos = Vec::with_capacity(4);
        for p in cached {
            photos.push(photo(
                p.original_sha256,
                p.version_sha256,
                p.width,
                p.height,
                p.rgba,
            )?);
        }
        Self::finish(photos)
    }
}

pub(super) struct NativeMaskImages {
    data: NativeMaskImageData,
    scope: ImageScope,
    _reservation: BaseBackingReservation,
    _heap: ImageMemoryLease,
    retained_data_bytes: usize,
}
impl NativeMaskImages {
    pub(super) fn initialize(
        host: &mut StdHost,
        provider: &NativeSurfaceProvider,
        initialization: NativeMaskImageInitialization,
    ) -> Result<Self, String> {
        let peak = initialization.preparation_bytes;
        let heap = host.kernel_resources.reserve_managed_base_memory(peak)?;
        let pool = provider
            .entry
            .resources
            .iter()
            .find(|p| p.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS)
            .ok_or("native image Mask pool absent")?;
        let reservation = host.kernel_resources.reserve_base_backing(
            &host.advertisement,
            &host.base_registry,
            &provider.entry.provider_instance_id,
            provider.entry.provider_generation,
            &pool.pool_id,
            u32::try_from(peak).map_err(|_| "native image preparation units")?,
        )?;
        if [
            host.advertisement.host_id.as_str(),
            host.advertisement.boot_id.as_str(),
            provider.entry.provider_instance_id.as_str(),
            pool.pool_id.as_str(),
        ]
        .iter()
        .any(|id| id.len() > 256)
        {
            return Err("native image scope metadata exceeds bound".into());
        }
        let data = (initialization.factory)()?;
        let metadata = 2 * core::mem::size_of::<usize>()
            + core::mem::size_of::<Self>()
            + ImageMemoryLease::owned_backing_bytes()
            + initialization.body.retained_backing_bytes()
            + 2 * (host.advertisement.host_id.as_str().len()
                + host.advertisement.boot_id.as_str().len()
                + provider.entry.provider_instance_id.as_str().len())
            + pool.pool_id.as_str().len()
            + RUNTIME_MEMORY_RESOURCE_CLASS.len();
        if [
            host.advertisement.host_id.as_str(),
            host.advertisement.boot_id.as_str(),
            provider.entry.provider_instance_id.as_str(),
        ]
        .iter()
        .any(|id| id.len() > 256)
            || data
                .owned_bytes()
                .checked_add(metadata)
                .is_none_or(|n| n > initialization.retained_bytes)
        {
            return Err("native image retained data exceeds admitted bound".into());
        }
        // Retain the entire peak reservation: the base pool has no partial
        // release operation. Never make live backing appear free.
        let retained_data_bytes = data.owned_bytes() + metadata;
        Ok(Self {
            data,
            retained_data_bytes,
            scope: ImageScope {
                body: initialization.body,
                host: host.advertisement.host_id.clone(),
                boot: host.advertisement.boot_id.clone(),
                provider: provider.entry.provider_instance_id.clone(),
                generation: provider.entry.provider_generation,
            },
            _reservation: reservation,
            _heap: heap,
        })
    }
    pub(super) fn receipt(&self) -> NativeMaskImageBackingReceipt {
        NativeMaskImageBackingReceipt {
            retained_data_bytes: self.retained_data_bytes,
            runtime_pool_bytes: self._reservation.bytes() as usize,
            image_heap_charge: self._reservation.bytes() as usize,
        }
    }
    pub(super) fn valid(&self, host: &StdHost) -> bool {
        self._reservation.matches_ledger(&host.kernel_resources)
            && self._reservation.matches_registry(&host.base_registry)
            && self.scope.host == host.advertisement.host_id
            && self.scope.boot == host.advertisement.boot_id
    }
    pub(super) fn views<'a>(
        &'a self,
        face: &Face,
        possession: &'a ActiveNativeSurfacePossession,
    ) -> Result<ScopedImageViews<'a>, DisplayError> {
        self.scope
            .views(&self.data, face.basis.body_id.as_ref(), possession)
    }
}
struct ImageScope {
    body: BodyId,
    host: HostId,
    boot: BootId,
    provider: BaseInstanceId,
    generation: u64,
}
impl ImageScope {
    fn views<'a>(
        &self,
        data: &'a NativeMaskImageData,
        body: Option<&BodyId>,
        possession: &'a ActiveNativeSurfacePossession,
    ) -> Result<ScopedImageViews<'a>, DisplayError> {
        let claim = possession
            .authorized_claim()
            .ok_or(DisplayError::ImageAccessDenied)?;
        if body != Some(&self.body)
            || claim.host_id != self.host
            || claim.boot_id != self.boot
            || claim.base_instance_id != self.provider
            || claim.base_provider_generation != self.generation
        {
            return Err(DisplayError::ImageAccessDenied);
        }
        Ok(ScopedImageViews {
            data,
            _possession: possession,
        })
    }
}

pub(super) struct ScopedImageViews<'a> {
    data: &'a NativeMaskImageData,
    _possession: &'a ActiveNativeSurfacePossession,
}
impl ImageResolver for ScopedImageViews<'_> {
    fn resolve(&self, key: GraphicsImageKey) -> Result<OpaqueRgbaImage<'_>, DisplayError> {
        let p = self
            .data
            .photos
            .iter()
            .find(|p| p.key.identity() == key.identity())
            .ok_or(DisplayError::ImageUnavailable)?;
        if p.key.version() != key.version() {
            return Err(DisplayError::ImageStale);
        }
        OpaqueRgbaImage::prepare(p.width, p.height, &p.rgba)
    }
}
#[cfg(test)]
#[path = "images/tests.rs"]
mod tests;
