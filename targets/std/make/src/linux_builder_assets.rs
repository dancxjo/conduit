//! Explicit local custody of the reviewed ConduitOS boot package. Source and
//! installed compiler grants do not authorize acquisition of external assets.
use conduit_host_make::{BuilderBounds, BuilderRefusal};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(super) const LIMINE_VERSION: &str = "12.5.2";
pub(super) const LIMINE_SHA256: &str =
    "4c760c09c53560d859b362319a3dc63b79cca3d47f35d69ab0106a13b8057055";
const LIMINE_URL: &str =
    "https://github.com/limine-bootloader/limine/releases/download/v12.5.2/limine-binary.tar.gz";

/// The owning admitted Host operation supplies this local read grant explicitly.
/// This grants no network acquisition. The archive is independently verified
/// while copying; a previously verified pathname is never trusted as bytes.
pub struct ReviewedConduitosBootAssets {
    limine_archive: PathBuf,
}
impl ReviewedConduitosBootAssets {
    pub fn local_limine_archive(path: PathBuf) -> Self {
        Self {
            limine_archive: path,
        }
    }

    pub(super) fn stage(
        &self,
        source: &Path,
        bounds: &BuilderBounds,
        progress: &mut dyn FnMut() -> Result<(), BuilderRefusal>,
    ) -> Result<(), BuilderRefusal> {
        progress()?;
        // Refuse a changed package acquisition contract instead of silently
        // applying this local asset grant to a different source's package pin.
        let profile = super::verification::read_bounded(
            &source.join("targets/conduitos/make/xtask/profile.rs"),
            bounds.maximum_source_bytes,
            BuilderRefusal::SourceBoundExhausted,
            progress,
        )?;
        let profile =
            std::str::from_utf8(&profile).map_err(|_| BuilderRefusal::UnsupportedAdapter)?;
        if !profile.contains(&format!(
            "pub const LIMINE_VERSION: &str = \"{LIMINE_VERSION}\";"
        )) || !profile.contains(&format!(
            "pub const LIMINE_ARCHIVE_SHA256: &str =\n    \"{LIMINE_SHA256}\";"
        )) || !profile.contains(&format!(
            "pub const LIMINE_ARCHIVE_URL: &str =\n    \"{LIMINE_URL}\";"
        )) {
            return Err(BuilderRefusal::UnsupportedAdapter);
        }
        let vendor = source.join("target/conduitos/vendor");
        fs::create_dir_all(&vendor).map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
        let destination = vendor.join(format!("limine-binary-{LIMINE_VERSION}.tar.gz"));
        let mut input =
            fs::File::open(&self.limine_archive).map_err(|_| BuilderRefusal::MissingSource)?;
        if !input
            .metadata()
            .map_err(|_| BuilderRefusal::MissingSource)?
            .is_file()
        {
            return Err(BuilderRefusal::MalformedArtifact);
        }
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
        let mut hasher = Sha256::new();
        let mut bytes = 0_u64;
        let mut buffer = [0; 65536];
        loop {
            progress()?;
            let n = input
                .read(&mut buffer)
                .map_err(|_| BuilderRefusal::MissingSource)?;
            if n == 0 {
                break;
            }
            bytes = bytes
                .checked_add(n as u64)
                .ok_or(BuilderRefusal::BoundExhausted)?;
            if bytes > bounds.maximum_workspace_bytes {
                return Err(BuilderRefusal::BoundExhausted);
            }
            hasher.update(&buffer[..n]);
            output
                .write_all(&buffer[..n])
                .map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
        }
        progress()?;
        if format!("{:x}", hasher.finalize()) != LIMINE_SHA256 {
            return Err(BuilderRefusal::MalformedArtifact);
        }
        output
            .flush()
            .map_err(|_| BuilderRefusal::WorkspaceUnavailable)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_asset_grant_rejects_unreviewed_bytes_without_acquisition() {
        let root =
            std::env::temp_dir().join(format!("conduit-linux-assets-{}", std::process::id()));
        let profile = root.join("targets/conduitos/make/xtask");
        fs::create_dir_all(&profile).unwrap();
        fs::write(profile.join("profile.rs"), format!("pub const LIMINE_VERSION: &str = \"{LIMINE_VERSION}\";\npub const LIMINE_ARCHIVE_SHA256: &str =\n    \"{LIMINE_SHA256}\";\npub const LIMINE_ARCHIVE_URL: &str =\n    \"{LIMINE_URL}\";\n")).unwrap();
        let archive = root.join("archive.tar.gz");
        fs::write(&archive, b"unreviewed archive").unwrap();
        let bounds = BuilderBounds {
            maximum_source_files: 10,
            maximum_source_bytes: 1024,
            maximum_workspace_bytes: 1024 * 1024,
            maximum_output_bytes: 1024,
            maximum_processes: 1,
            maximum_seconds: 1,
            maximum_evidence_bytes: 1024,
        };
        assert_eq!(
            ReviewedConduitosBootAssets::local_limine_archive(archive).stage(
                &root,
                &bounds,
                &mut || Ok(())
            ),
            Err(BuilderRefusal::MalformedArtifact)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
