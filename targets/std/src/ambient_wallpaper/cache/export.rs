//! Explicit operator preparation effect. A cache contains data, never Body or
//! membership authority, and is not read by the Mask until Root admission.
use super::*;
use std::{io::Write, os::fd::FromRawFd};

/// Export exactly the acquired deck as raw opaque RGBA and a checksummed
/// manifest. Refuse an existing destination; preserve any partial export
/// on failure for explicit operator inspection. No acquisition occurs here.
pub fn write_deck(
    directory: &Path,
    deck: &crate::ambient_wallpaper::acquisition::PreparedDeck,
) -> Result<[u8; 32], String> {
    if directory.as_os_str().len() > 2048 {
        return Err("cache export path bound".into());
    }
    let mut entries = Vec::with_capacity(4);
    for p in deck.photos() {
        let width = u16::try_from(p.width()).map_err(|_| "cache width")?;
        let height = u16::try_from(p.height()).map_err(|_| "cache height")?;
        if width > 1024
            || height > 600
            || version(p.original_sha256(), width, height, p.rgba()) != *p.version_sha256()
        {
            return Err("cache exact prepared extent/version differs".into());
        }
        if width == 0
            || height == 0
            || p.rgba().len() != usize::from(width) * usize::from(height) * 4
            || p.rgba()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] != 255)
        {
            return Err("cache requires opaque prepared RGBA".into());
        }
        let suffix: String = p
            .version_sha256()
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect();
        entries.push(ManifestPhoto {
            file: format!("{suffix}.rgba"),
            width,
            height,
            original_sha256: *p.original_sha256(),
            version_sha256: *p.version_sha256(),
            rgba_sha256: Sha256::digest(p.rgba()).into(),
            provenance: Some(ManifestProvenance {
                date: p.candidate().date.clone(),
                title: p.candidate().title.clone(),
                asset_url: p.candidate().asset_url.clone(),
                credit: p.candidate().credit.clone(),
            }),
        });
    }
    let manifest = Manifest {
        photos: entries
            .try_into()
            .map_err(|_| "cache requires four photographs")?,
    };
    let bytes = serde_json::to_vec(&manifest).map_err(|_| "cache manifest encoding")?;
    if bytes.len() > MAXIMUM_MANIFEST_BYTES {
        return Err("cache manifest bound".into());
    }
    let parent = open_directory(directory.parent().unwrap_or(Path::new(".")))?;
    let name = directory
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("cache directory basename")?;
    if name.len() > 128 || name.is_empty() {
        return Err("cache directory basename bound".into());
    }
    let name = std::ffi::CString::new(name).map_err(|_| "cache directory basename")?;
    // SAFETY: held parent FD and valid single basename; mkdirat refuses an
    // existing target, including a symlink. The operator owns this parent.
    if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
        return Err("cache export destination already exists or is unavailable".into());
    }
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err("cache export directory unavailable".into());
    }
    let destination = unsafe { File::from_raw_fd(fd) };
    for (p, entry) in deck.photos().iter().zip(&manifest.photos) {
        write_new(&destination, &entry.file, p.rgba())?;
    }
    write_new(&destination, "manifest.json", &bytes)?;
    destination
        .sync_all()
        .map_err(|_| "cache export directory sync")?;
    Ok(Sha256::digest(&bytes).into())
}
fn write_new(directory: &File, name: &str, bytes: &[u8]) -> Result<(), String> {
    let name = std::ffi::CString::new(name).map_err(|_| "cache filename")?;
    // SAFETY: basename is generated internally; EXCL refuses every existing
    // target and the returned descriptor has unique owned File custody.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o400,
        )
    };
    if fd < 0 {
        return Err("cache export file already exists or is unavailable".into());
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bytes).map_err(|_| "cache export write")?;
    file.sync_all().map_err(|_| "cache export file sync".into())
}
