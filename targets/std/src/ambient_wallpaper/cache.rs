//! Checksummed immutable photo cache. Read/write are explicit operator or Root
//! preparation effects; cached IDs, provenance and pixels mint no authority.
use super::{
    ApodImageCandidate,
    acquisition::{PreparedDeck, PreparedPhoto},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    os::{fd::AsRawFd, unix::ffi::OsStrExt},
    path::Path,
};
mod export;
pub use export::write_deck;
pub(crate) const MAXIMUM_MANIFEST_BYTES: usize = 64 * 1024;
const MAXIMUM_PIXELS: usize = 1024 * 600;
pub const CACHE_PREPARATION_BYTES: usize = 12 * 1024 * 1024;

pub(crate) struct CachedPhoto {
    pub candidate: Option<ApodImageCandidate>,
    pub width: u16,
    pub height: u16,
    pub rgba: Box<[u8]>,
    pub original_sha256: [u8; 32],
    pub version_sha256: [u8; 32],
}
/// Reconstruct a Source deck without network or image decode. All four entries
/// must retain checked actual APOD provenance. Licenses remain unknown.
/// Caller separately admits CACHE_PREPARATION_BYTES before entering this effect.
pub fn load_deck(path: &Path, expected_sha256: [u8; 32]) -> Result<PreparedDeck, String> {
    let cached = load_photos(path, expected_sha256)?;
    let mut photos = Vec::with_capacity(4);
    for p in cached {
        let candidate = p
            .candidate
            .ok_or("source cache requires actual APOD provenance")?;
        photos.push(PreparedPhoto::from_checked_cache(
            candidate,
            u32::from(p.width),
            u32::from(p.height),
            p.rgba,
            p.original_sha256,
            p.version_sha256,
        ));
    }
    Ok(PreparedDeck::from_checked_cache(
        photos
            .try_into()
            .map_err(|_| "source cache requires four photographs")?,
    ))
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub(crate) photos: [ManifestPhoto; 4],
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManifestPhoto {
    pub(crate) file: String,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) original_sha256: [u8; 32],
    pub(crate) version_sha256: [u8; 32],
    pub(crate) rgba_sha256: [u8; 32],
    pub(crate) provenance: Option<ManifestProvenance>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManifestProvenance {
    pub(crate) date: String,
    pub(crate) title: String,
    pub(crate) asset_url: String,
    pub(crate) credit: Option<String>,
}
pub(crate) fn version(original: &[u8; 32], width: u16, height: u16, rgba: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"conduit-ambient-rgba-v1\0");
    digest.update(original);
    digest.update(u32::from(width).to_le_bytes());
    digest.update(u32::from(height).to_le_bytes());
    digest.update(rgba);
    digest.finalize().into()
}

pub(crate) fn load_photos(
    path: &Path,
    expected_manifest_sha256: [u8; 32],
) -> Result<[CachedPhoto; 4], String> {
    if path.as_os_str().len() > 2048 {
        return Err("native image manifest path too long".into());
    }
    let name = path
        .file_name()
        .and_then(|p| p.to_str())
        .ok_or("native image manifest basename")?;
    let directory = open_directory(path.parent().unwrap_or(Path::new(".")))?;
    let mut manifest = open_relative(&directory, name)?;
    let length = usize::try_from(manifest.metadata().map_err(|_| "manifest metadata")?.len())
        .map_err(|_| "manifest length")?;
    if length == 0 || length > MAXIMUM_MANIFEST_BYTES {
        return Err("native image manifest bound".into());
    }
    let mut bytes = vec![0; length];
    exact_read(&mut manifest, &mut bytes)?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != expected_manifest_sha256 {
        return Err("native image manifest checksum differs".into());
    }
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|_| "native image manifest invalid")?;
    let mut photos = Vec::with_capacity(4);
    for p in manifest.photos {
        let candidate = if let Some(source) = &p.provenance {
            let bytes =
                serde_json::to_vec(&serde_json::json!({"date":source.date,"title":source.title,
                    "hdurl":source.asset_url,"copyright":source.credit,"media_type":"image"}))
                .map_err(|_| "cache provenance encoding")?;
            let candidate =
                super::decode_apod_candidate(&bytes).map_err(|_| "cache provenance invalid")?;
            if source.credit.as_ref().is_some_and(|c| c.len() > 256) {
                return Err("cache provenance credit bound".into());
            }
            Some(candidate)
        } else {
            None
        };
        let count = usize::from(p.width) * usize::from(p.height);
        if p.width > 1024 || p.height > 600 || count == 0 || count > MAXIMUM_PIXELS {
            return Err("native image extent exceeds bound".into());
        }
        let mut file = open_relative(&directory, &p.file)?;
        if file.metadata().map_err(|_| "native image metadata")?.len() != (count * 4) as u64 {
            return Err("native image length differs".into());
        }
        let mut rgba = vec![0; count * 4].into_boxed_slice();
        exact_read(&mut file, &mut rgba)?;
        if <[u8; 32]>::from(Sha256::digest(&rgba)) != p.rgba_sha256 {
            return Err("native image checksum differs".into());
        }
        if p.original_sha256 == [0; 32]
            || p.version_sha256 == [0; 32]
            || rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 255)
            || version(&p.original_sha256, p.width, p.height, &rgba) != p.version_sha256
            || photos
                .iter()
                .any(|other: &CachedPhoto| other.original_sha256 == p.original_sha256)
        {
            return Err("cache exact immutable pixels/version differ".into());
        }
        photos.push(CachedPhoto {
            candidate,
            width: p.width,
            height: p.height,
            rgba,
            original_sha256: p.original_sha256,
            version_sha256: p.version_sha256,
        });
    }
    photos
        .try_into()
        .map_err(|_| "cache requires four photographs".into())
}
fn open_directory(path: &Path) -> Result<File, String> {
    use std::{os::fd::FromRawFd, path::Component};
    let mut directory = File::open(if path.is_absolute() { "/" } else { "." })
        .map_err(|_| "native image directory root unavailable")?;
    for (index, component) in path.components().enumerate() {
        if index >= 256 {
            return Err("native image directory component bound".into());
        }
        let name = match component {
            Component::RootDir | Component::CurDir => continue,
            Component::Normal(name) => name,
            _ => return Err("native image directory traversal refused".into()),
        };
        let name = std::ffi::CString::new(name.as_bytes())
            .map_err(|_| "native image directory component")?;
        // SAFETY: each immutable parent FD anchors one basename open; every
        // component forbids symlinks and owns its returned descriptor exactly once.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err("native image directory unavailable".into());
        }
        directory = unsafe { File::from_raw_fd(fd) };
    }
    Ok(directory)
}
fn open_relative(directory: &File, name: &str) -> Result<File, String> {
    if name.is_empty()
        || name.len() > 128
        || matches!(name, "." | "..")
        || name.contains(['/', '\\', '\0'])
    {
        return Err("native image cache path is not a basename".into());
    }
    let name = std::ffi::CString::new(name).map_err(|_| "native image cache basename")?;
    // SAFETY: held directory and valid NUL-terminated basename; returned FD has
    // unique File custody. No symlink is followed and no device/FIFO is read.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err("native image cache file unavailable".into());
    }
    use std::os::fd::FromRawFd;
    let file = unsafe { File::from_raw_fd(fd) };
    if !file
        .metadata()
        .map_err(|_| "native image file metadata")?
        .is_file()
    {
        return Err("native image cache requires regular files".into());
    }
    Ok(file)
}
fn exact_read(file: &mut File, bytes: &mut [u8]) -> Result<(), String> {
    file.read_exact(bytes)
        .map_err(|_| "native image cache truncated")?;
    if file
        .read(&mut [0u8; 1])
        .map_err(|_| "native image cache read")?
        != 0
    {
        return Err("native image cache grew".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "cache/tests.rs"]
mod tests;
