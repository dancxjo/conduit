//! Retained prior Make payloads and private output custody for bundle Seeds.
use super::*;
use conduit_host_browser_make::VerifiedBrowserBundleImage;
use conduit_host_make::{HostReleaseBounds, ReleaseTargetProfile, VerifiedHostReleaseBytes};
use std::{
    collections::BTreeMap,
    io::{Seek, SeekFrom},
    path::PathBuf,
    sync::atomic::Ordering,
};

enum Image {
    Native(Box<VerifiedHostReleaseBytes>),
    Browser(Box<VerifiedBrowserBundleImage>),
}
/// Actual opened payloads remain owned until production/drain. A verified
/// inventory alone is not a package and cannot substitute for these bytes.
pub struct CrecheReviewedBundle {
    image: Image,
    files: BTreeMap<String, File>,
    directory: PathBuf,
}
impl CrecheReviewedBundle {
    pub fn native(
        release: VerifiedHostReleaseBytes,
        files: BTreeMap<String, File>,
        directory: PathBuf,
    ) -> Result<Self, String> {
        Self::new(Image::Native(Box::new(release)), files, directory)
    }
    pub fn browser(
        image: VerifiedBrowserBundleImage,
        files: BTreeMap<String, File>,
        directory: PathBuf,
    ) -> Result<Self, String> {
        Self::new(Image::Browser(Box::new(image)), files, directory)
    }
    fn new(
        image: Image,
        files: BTreeMap<String, File>,
        directory: PathBuf,
    ) -> Result<Self, String> {
        let bundle = Self {
            image,
            files,
            directory,
        };
        let release = bundle.release();
        let m = release.manifest();
        if !bundle.directory.is_absolute()
            || !bundle.directory.is_dir()
            || bundle.files.len() != m.files.len()
            || m.files.len() > 16
            || m.files.iter().any(|f| !bundle.files.contains_key(&f.path))
        {
            return Err("Crèche bundle lacks exact payload/output custody".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata =
                std::fs::symlink_metadata(&bundle.directory).map_err(|e| e.to_string())?;
            if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
                return Err("Crèche package output directory must be private".into());
            }
        }
        let expected = ReleaseTargetProfile {
            target_id: &m.target_id,
            package_id: &m.make_package_id,
            output: &m.output,
            builder_adapter: &m.builder_adapter,
            deployment_adapter: &m.deployment_adapter,
        };
        // Reobserve the actual retained handles before the provider is offered.
        conduit_host_make::verify_host_release_bytes(
            release.manifest_bytes(),
            &expected,
            &m.source_identity,
            HostReleaseBounds {
                maximum_manifest_bytes: 256 * 1024,
                maximum_files: 16,
                maximum_file_bytes: 64 * 1024 * 1024,
                maximum_bundle_bytes: 64 * 1024 * 1024,
            },
            |name| bundle.open(name),
            || false,
        )?;
        Ok(bundle)
    }
    pub(super) fn release(&self) -> &VerifiedHostReleaseBytes {
        match &self.image {
            Image::Native(r) => r,
            Image::Browser(b) => b.release(),
        }
    }
    fn open(&self, name: &str) -> Result<File, String> {
        let mut file = self
            .files
            .get(name)
            .ok_or("Crèche payload not retained")?
            .try_clone()
            .map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("Crèche payload is not a regular file".into());
        }
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        Ok(file)
    }
    pub(super) fn validate(
        &self,
        a: &CrecheReviewedArtifact,
        bounds: JoiningProductionBounds,
    ) -> Result<(), String> {
        let m = self.release().manifest();
        if self.maximum_package_bytes()? > bounds.maximum_artifact_bytes {
            return Err("Crèche package workspace exceeds admitted artifact bytes".into());
        }
        if serde_json::to_value(&a.output)
            .map_err(|e| e.to_string())?
            .as_str()
            != Some(m.output.as_str())
            || m.source_identity != a.source_identity
            || m.target_id != a.image.manifest.target
            || m.make_package_id != a.image.manifest.make_package_id
            || m.builder_adapter != a.image.manifest.builder_adapter
            || Some(m.deployment_adapter.as_str()) != a.image.manifest.deployment_adapter.as_deref()
        {
            return Err("Crèche bundle source/target differs from selected IMAGE".into());
        }
        match &self.image {
            Image::Native(_) if a.output == SporeOutputKind::NativeBundle => {}
            Image::Browser(b)
                if a.output == SporeOutputKind::BrowserBundle
                    && b.generic_image_id() == a.image.manifest.image_id
                    && b.configuration_id() == a.configuration.configuration_id()
                    && b.profile_id() == a.image.manifest.profile_id => {}
            _ => return Err("Crèche bundle lacks exact target IMAGE proof".into()),
        }
        Ok(())
    }
    fn maximum_package_bytes(&self) -> Result<u64, String> {
        self.release()
            .payload_bytes()
            .checked_add(64 * 1024 + 17 * (30 + 46 + 512) + 22)
            .ok_or_else(|| "Crèche package size overflow".into())
    }
    pub(super) fn produce(
        &self,
        a: &CrecheReviewedArtifact,
        configuration: &CheckedHostConfiguration,
        binding: &CrecheBodyBinding,
        inputs: &Inputs,
        request: CrecheRequest,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<ProducedJoiningHost<File>, String> {
        if self.maximum_package_bytes()?
            > inputs.installation.production_bounds.maximum_artifact_bytes
        {
            return Err("Crèche package workspace exceeds admitted artifact bytes".into());
        }
        let spore = conduit_body_make::seal_joining_host_release(
            conduit_body_make::JoiningReleaseSelection {
                body: &binding.biography,
                invitation: &binding.invitation.claim,
                configuration,
                output: a.output.clone(),
                image: &a.image,
                image_manifest: &a.image_manifest,
                release: self.release(),
            },
            &inputs.catalog,
            &inputs.packages,
            clock()?,
        )?;
        let name = format!("seed-{:x}.zip", Sha256::digest(request.0));
        let path = self.directory.join(name);
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options
            .open(&path)
            .map_err(|e| format!("Crèche private package staging: {e}"))?;
        // Unlink before writing: partial and completed private bytes stay with
        // this opened handle, never appear as a publicly available package.
        if let Err(error) = std::fs::remove_file(&path) {
            drop(output);
            return Err(format!("Crèche private staging unlink: {error}"));
        }
        let receipt = conduit_body_make::write_joining_host_package(
            self.release(),
            &spore,
            configuration,
            &binding.invitation,
            clock()?,
            |name| self.open(name),
            &mut output,
            || cancelled.load(Ordering::Acquire),
        )?;
        if receipt.artifact_bytes > inputs.installation.production_bounds.maximum_artifact_bytes {
            return Err("Crèche package exceeds admitted artifact bytes".into());
        }
        binding
            .invitation
            .validate(clock()?)
            .map_err(|e| format!("Crèche package invitation expired: {e}"))?;
        if cancelled.load(Ordering::Acquire) {
            return Err("Crèche package production cancelled".into());
        }
        output
            .sync_all()
            .map_err(|e| format!("Crèche package flush: {e}"))?;
        output.rewind().map_err(|e| e.to_string())?;
        let spore_manifest = conduit_body_make::encode_joining_host_evidence(
            &spore.manifest,
            inputs.installation.production_bounds.maximum_manifest_bytes,
        )?;
        let receipt = conduit_body_make::encode_joining_host_evidence(
            &receipt,
            inputs.installation.production_bounds.maximum_receipt_bytes,
        )?;
        Ok(ProducedJoiningHost {
            invitation: binding.invitation.claim.clone(),
            artifact: output,
            spore,
            spore_manifest,
            receipt,
        })
    }
}
