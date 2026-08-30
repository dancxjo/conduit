use std::{fs, path::Path};

use sha2::{Digest, Sha256};

use super::{
    rust_firmware::{AVR_HAL_REVISION, RUST_TOOLCHAIN},
    ARDUINO_AVR_VERSION, CLI_VERSION, FQBN, SPARKFUN_AVR_VERSION,
};

pub(super) const BUILD_ID_SCHEMA: &str = "conduit.avr-promicro/build-id@1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EmbeddedBuildIdentity {
    pub(super) source_sha: String,
    pub(super) source_digest_sha256: String,
    pub(super) build_id: String,
    pub(super) profile: &'static str,
}

impl EmbeddedBuildIdentity {
    pub(super) fn new(
        source_sha: String,
        source_digest_sha256: String,
        profile: &'static str,
    ) -> Self {
        let canonical = format!(
            "schema={BUILD_ID_SCHEMA}\nsource_sha={source_sha}\nsource_digest_sha256={source_digest_sha256}\nprofile={profile}\ntarget={FQBN}\nrust_toolchain={RUST_TOOLCHAIN}\navr_hal={AVR_HAL_REVISION}\narduino_cli={CLI_VERSION}\narduino_avr={ARDUINO_AVR_VERSION}\nsparkfun_avr={SPARKFUN_AVR_VERSION}\n"
        );
        let build_id = format!("{:x}", Sha256::digest(canonical.as_bytes()));
        Self {
            source_sha,
            source_digest_sha256,
            build_id,
            profile,
        }
    }
}

pub(super) fn digest_compiled_sources(firmware: &Path) -> Result<String, std::io::Error> {
    let mut sources = Vec::new();
    collect_sources(firmware, &mut sources)?;
    if sources.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "AVR firmware contains no compiled sources",
        ));
    }
    sources.sort();

    let mut digest = Sha256::new();
    for source in sources {
        let name = source
            .strip_prefix(firmware)
            .map_err(std::io::Error::other)?
            .to_str()
            .ok_or_else(|| std::io::Error::other("AVR source path is not UTF-8"))?;
        let contents = fs::read(&source)?;
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update((contents.len() as u64).to_le_bytes());
        digest.update(contents);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn collect_sources(directory: &Path, sources: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) != Some("target") {
                collect_sources(&path, sources)?;
            }
        } else if matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml")
        ) || matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "toml")
        ) {
            sources.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_identity_is_exact_and_profile_specific() {
        let isolated = EmbeddedBuildIdentity::new("a".repeat(40), "b".repeat(64), "receive-only");
        let hil =
            EmbeddedBuildIdentity::new("a".repeat(40), "b".repeat(64), "assigned-create-host");
        assert_eq!(isolated.build_id.len(), 64);
        assert!(isolated
            .build_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
        assert_ne!(isolated.build_id, hil.build_id);
    }
}
