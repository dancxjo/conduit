//! Explicit checked-program fabrication input, separate from a private Seed.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "host_workset_specializations.rs"]
mod specializations;
const MAXIMUM_SOURCE_BYTES: u64 = 1024 * 1024;
pub(super) struct WorksetSourceInput {
    pub source: PathBuf,
    pub sha256: String,
    pub entries: Vec<String>,
    pub git_revision: String,
    pub identity: String,
    specializations: Option<specializations::Captured>,
}
impl WorksetSourceInput {
    pub fn capture(
        source: &Path,
        entries: &[String],
        specialization_input: Option<&Path>,
        git_revision: &str,
        output: &Path,
        dry_run: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if entries.is_empty()
            || entries.len() > conduit_body::MAX_BODY_PLOTS
            || entries
                .iter()
                .any(|entry| entry.is_empty() || entry.len() > 256)
            || entries
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != entries.len()
        {
            return Err(
                "fabrication Source needs unique explicitly selected finite entries".into(),
            );
        }
        let mut bytes = Vec::new();
        fs::File::open(source)?
            .take(MAXIMUM_SOURCE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.is_empty() || bytes.len() as u64 > MAXIMUM_SOURCE_BYTES {
            return Err("fabrication Source extent refused".into());
        }
        std::str::from_utf8(&bytes)?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let specializations = specialization_input
            .map(|path| specializations::Captured::capture(path, &sha256, output, dry_run))
            .transpose()?;
        let mut digest = Sha256::new();
        if let Some(input) = &specializations {
            digest.update(b"conduit.make/workset-source@2\0");
            digest.update((input.sha256.len() as u32).to_le_bytes());
            digest.update(input.sha256.as_bytes());
        } else {
            digest.update(b"conduit.make/workset-source@1\0");
        }
        for part in std::iter::once(git_revision)
            .chain(std::iter::once(sha256.as_str()))
            .chain(entries.iter().map(String::as_str))
        {
            digest.update((part.len() as u32).to_le_bytes());
            digest.update(part.as_bytes());
        }
        let identity = format!("sha256:{:x}", digest.finalize());
        let path = if dry_run {
            source.canonicalize()?
        } else {
            fs::create_dir_all(output)?;
            let path = output.join("workset-source.conduit");
            fs::write(&path, bytes)?;
            path.canonicalize()?
        };
        Ok(Self {
            source: path,
            sha256,
            entries: entries.to_vec(),
            git_revision: git_revision.into(),
            identity,
            specializations,
        })
    }
    pub fn configure(&self, command: &mut Command) -> Result<(), Box<dyn std::error::Error>> {
        self.verify()?;
        clear_inputs(command);
        if let Some(input) = &self.specializations {
            input.configure(command);
        }
        command
            .env("CONDUIT_WORKSET_SOURCE", &self.source)
            .env("CONDUIT_WORKSET_SOURCE_SHA256", &self.sha256)
            .env(
                "CONDUIT_WORKSET_ENTRIES",
                serde_json::to_string(&self.entries)?,
            );
        Ok(())
    }
    pub fn verify(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = fs::File::open(&self.source)?.take(MAXIMUM_SOURCE_BYTES + 1);
        let mut digest = Sha256::new();
        let mut buffer = [0; 8192];
        let mut extent = 0;
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            extent += n;
            if extent as u64 > MAXIMUM_SOURCE_BYTES {
                return Err("captured fabrication Source grew".into());
            }
            digest.update(&buffer[..n]);
        }
        if format!("{:x}", digest.finalize()) != self.sha256 {
            return Err("captured fabrication Source changed during build".into());
        }
        if let Some(input) = &self.specializations {
            input.verify(&self.sha256)?;
        }
        Ok(())
    }
    pub fn publish(&self, artifact_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.verify()?;
        let mut artifact: conduit_host_make::HostReleaseManifest =
            serde_json::from_slice(&fs::read(artifact_path)?)?;
        artifact.target_metadata.insert("conduit.make/workset-source@1".into(), serde_json::json!({
            "git_revision":self.git_revision,"sha256":self.sha256,"entries":self.entries,"identity":self.identity,
        }));
        if let Some(input) = &self.specializations {
            artifact.target_metadata.insert(
                "conduit.make/workset-specializations@1".into(),
                serde_json::json!({"sha256":input.sha256,"source_sha256":self.sha256}),
            );
        }
        fs::write(artifact_path, serde_json::to_vec_pretty(&artifact)?)?;
        Ok(())
    }
}

/// A parent shell cannot add compilation inputs absent from the Make result.
pub(super) fn clear_inputs(command: &mut Command) {
    for name in [
        "CONDUIT_WORKSET_SOURCE",
        "CONDUIT_WORKSET_SOURCE_SHA256",
        "CONDUIT_WORKSET_ENTRIES",
        "CONDUIT_WORKSET_SPECIALIZATIONS",
        "CONDUIT_WORKSET_SPECIALIZATIONS_SHA256",
    ] {
        command.env_remove(name);
    }
}

/// Capture the original public Make IMAGE before its target child builder runs.
/// Calibration must consume this exact object, not reconstruct a substitute.
pub(super) struct ImageDescriptionInput {
    path: PathBuf,
    digest: String,
}
impl ImageDescriptionInput {
    pub fn capture(
        output: &Path,
        bytes: &[u8],
        image: &conduit_host_make::HostImage,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if bytes.is_empty() || bytes.len() as u64 > MAXIMUM_SOURCE_BYTES {
            return Err("public IMAGE description extent refused".into());
        }
        let decoded: conduit_host_make::HostImage = serde_json::from_slice(bytes)?;
        if &decoded != image {
            return Err("public IMAGE description differs from original Make result".into());
        }
        fs::create_dir_all(output)?;
        let path = output.join("resolved-image.json");
        fs::write(&path, bytes)?;
        Ok(Self {
            path: path.canonicalize()?,
            digest: format!("{:x}", Sha256::digest(bytes)),
        })
    }
    pub fn configure(&self, command: &mut Command) -> Result<(), Box<dyn std::error::Error>> {
        self.verify()?;
        command
            .env("CONDUIT_HOST_IMAGE_DESCRIPTION", &self.path)
            .env("CONDUIT_HOST_IMAGE_DESCRIPTION_SHA256", &self.digest);
        Ok(())
    }
    pub fn verify(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        fs::File::open(&self.path)?
            .take(MAXIMUM_SOURCE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAXIMUM_SOURCE_BYTES
            || format!("{:x}", Sha256::digest(&bytes)) != self.digest
        {
            return Err("captured public IMAGE changed during fabrication".into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "host_workset_source_tests.rs"]
mod tests;
