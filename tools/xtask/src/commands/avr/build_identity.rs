use std::{fs, path::Path};

use sha2::{Digest, Sha256};

use super::{ARDUINO_AVR_VERSION, CLI_VERSION, FQBN, SPARKFUN_AVR_VERSION};

pub(super) const BUILD_ID_SCHEMA: &str = "conduit.avr-promicro/build-id@1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EmbeddedBuildIdentity {
    pub(super) source_sha: String,
    pub(super) source_digest_sha256: String,
    pub(super) build_id: String,
    pub(super) profile: &'static str,
}

impl EmbeddedBuildIdentity {
    pub(super) fn new(source_sha: String, source_digest_sha256: String, create_hil: bool) -> Self {
        let profile = if create_hil { "create-hil" } else { "isolated" };
        let canonical = format!(
            "schema={BUILD_ID_SCHEMA}\nsource_sha={source_sha}\nsource_digest_sha256={source_digest_sha256}\nprofile={profile}\ntarget={FQBN}\narduino_cli={CLI_VERSION}\narduino_avr={ARDUINO_AVR_VERSION}\nsparkfun_avr={SPARKFUN_AVR_VERSION}\n"
        );
        let build_id = format!("{:x}", Sha256::digest(canonical.as_bytes()));
        Self {
            source_sha,
            source_digest_sha256,
            build_id,
            profile,
        }
    }

    pub(super) fn header(&self) -> String {
        format!(
            "#pragma once\n#define CONDUIT_AVR_BUILD_ID \"{}\"\n#define CONDUIT_AVR_SOURCE_SHA \"{}\"\n#define CONDUIT_AVR_SOURCE_DIGEST \"{}\"\n",
            self.build_id, self.source_sha, self.source_digest_sha256
        )
    }

    pub(super) fn compiler_flags(&self, header: &Path, create_hil: bool) -> String {
        let mut flags = format!("-include{}", header.display());
        if create_hil {
            flags.push_str(" -DCONDUIT_CREATE_HIL=1");
        }
        flags
    }
}

pub(super) fn digest_compiled_sources(sketch: &Path) -> Result<String, std::io::Error> {
    let mut sources = Vec::new();
    for entry in fs::read_dir(sketch)? {
        let path = entry?.path();
        if path.is_file()
            && matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("h" | "ino")
            )
        {
            sources.push(path);
        }
    }
    if sources.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "AVR sketch contains no compiled sources",
        ));
    }
    sources.sort();

    let mut digest = Sha256::new();
    for source in sources {
        debug_assert!(matches!(
            source.extension().and_then(|extension| extension.to_str()),
            Some("h" | "ino")
        ));
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| std::io::Error::other("AVR source name is not UTF-8"))?;
        let contents = fs::read(&source)?;
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update((contents.len() as u64).to_le_bytes());
        digest.update(contents);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_identity_is_exact_and_profile_specific() {
        let isolated = EmbeddedBuildIdentity::new("a".repeat(40), "b".repeat(64), false);
        let hil = EmbeddedBuildIdentity::new("a".repeat(40), "b".repeat(64), true);
        assert_eq!(isolated.build_id.len(), 64);
        assert!(isolated
            .build_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
        assert_ne!(isolated.build_id, hil.build_id);
        assert!(isolated.header().contains(&isolated.build_id));
        assert!(isolated.header().contains(&isolated.source_sha));
        assert!(isolated.header().contains(&isolated.source_digest_sha256));
        assert!(!isolated
            .compiler_flags(Path::new("/tmp/identity.h"), false)
            .contains("CONDUIT_CREATE_HIL"));
        assert!(hil
            .compiler_flags(Path::new("/tmp/identity.h"), true)
            .contains("-DCONDUIT_CREATE_HIL=1"));
    }
}
