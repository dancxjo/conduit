//! Bounded canonical authored-program capsules.
//!
//! A capsule carries authored source and explicit lock/reference facts. It is
//! never an exact plan, host observation, grant, secret, or run archive.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub const CAPSULE_SCHEMA: &str = "conduit.panel-capsule";
pub const CAPSULE_SCHEMA_VERSION: u16 = 0;
pub const MAXIMUM_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAXIMUM_AUXILIARY_BYTES: usize = 1024 * 1024;
pub const MAXIMUM_REFERENCES: usize = 256;
pub const MAXIMUM_CAPSULE_DOCUMENT_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapsuleDocument {
    pub schema: String,
    pub schema_version: u16,
    pub identity: String,
    pub program_identity: String,
    pub source_revision: String,
    pub source_semantic_identity: String,
    pub source: String,
    pub import_lock: Option<InlineDocument>,
    pub presentation: Option<InlineDocument>,
    pub artifact_references: Vec<ArtifactReference>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InlineDocument {
    pub media_type: String,
    pub digest: String,
    pub text: String,
    pub sensitivity: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReference {
    pub digest: String,
    pub byte_size: u64,
    pub media_type: String,
    pub license: String,
    pub provenance: String,
    pub sensitivity: String,
    pub acquisition: String,
    pub embedded: bool,
}

#[derive(Serialize)]
struct ProgramProjection<'a> {
    schema: &'a str,
    schema_version: u16,
    source_revision: &'a str,
    source_semantic_identity: &'a str,
    import_lock_digest: Option<&'a str>,
    artifact_references: &'a [ArtifactReference],
}

#[derive(Serialize)]
struct CapsuleProjection<'a> {
    program_identity: &'a str,
    presentation_digest: Option<&'a str>,
}

impl CapsuleDocument {
    pub fn new(
        source: String,
        import_lock: Option<InlineDocument>,
        presentation: Option<InlineDocument>,
        mut artifact_references: Vec<ArtifactReference>,
    ) -> Result<Self, CapsuleError> {
        let panel = conduit_panel::parse(&source).map_err(|_| CapsuleError::InvalidSource)?;
        artifact_references.sort();
        let mut document = Self {
            schema: CAPSULE_SCHEMA.to_owned(),
            schema_version: CAPSULE_SCHEMA_VERSION,
            identity: String::new(),
            program_identity: String::new(),
            source_revision: digest(source.as_bytes()),
            source_semantic_identity: conduit_panel::semantic_source_hash(&panel),
            source,
            import_lock,
            presentation,
            artifact_references,
        };
        document.seal()?;
        Ok(document)
    }

    pub fn seal(&mut self) -> Result<(), CapsuleError> {
        self.artifact_references.sort();
        self.program_identity = self.computed_program_identity()?;
        self.identity = self.computed_identity()?;
        self.validate()
    }

    pub fn validate(&self) -> Result<(), CapsuleError> {
        if self.schema != CAPSULE_SCHEMA || self.schema_version != CAPSULE_SCHEMA_VERSION {
            return Err(CapsuleError::UnsupportedVersion);
        }
        if self.source.len() > MAXIMUM_SOURCE_BYTES
            || self.artifact_references.len() > MAXIMUM_REFERENCES
        {
            return Err(CapsuleError::LimitExceeded);
        }
        let panel = conduit_panel::parse(&self.source).map_err(|_| CapsuleError::InvalidSource)?;
        if self.source_revision != digest(self.source.as_bytes())
            || self.source_semantic_identity != conduit_panel::semantic_source_hash(&panel)
            || self.program_identity != self.computed_program_identity()?
            || self.identity != self.computed_identity()?
        {
            return Err(CapsuleError::IdentityMismatch);
        }
        validate_inline(
            self.import_lock.as_ref(),
            "application/vnd.conduit.contract-lock+json",
        )?;
        validate_inline(
            self.presentation.as_ref(),
            "application/vnd.conduit.presentation+json",
        )?;
        let mut digests = BTreeSet::new();
        for reference in &self.artifact_references {
            if !valid_digest(&reference.digest)
                || reference.byte_size == 0
                || reference.media_type.is_empty()
                || reference.license.is_empty()
                || reference.provenance.is_empty()
                || !matches!(
                    reference.sensitivity.as_str(),
                    "public" | "restricted" | "secret"
                )
                || !matches!(
                    reference.acquisition.as_str(),
                    "never" | "explicit" | "embedded"
                )
                || reference.embedded != (reference.acquisition == "embedded")
                || !digests.insert(reference.digest.as_str())
            {
                return Err(CapsuleError::InvalidReference);
            }
        }
        Ok(())
    }

    pub fn computed_program_identity(&self) -> Result<String, CapsuleError> {
        let bytes = serde_json::to_vec(&ProgramProjection {
            schema: &self.schema,
            schema_version: self.schema_version,
            source_revision: &self.source_revision,
            source_semantic_identity: &self.source_semantic_identity,
            import_lock_digest: self.import_lock.as_ref().map(|lock| lock.digest.as_str()),
            artifact_references: &self.artifact_references,
        })
        .map_err(|_| CapsuleError::Malformed)?;
        Ok(digest(&bytes))
    }

    pub fn computed_identity(&self) -> Result<String, CapsuleError> {
        let bytes = serde_json::to_vec(&CapsuleProjection {
            program_identity: &self.program_identity,
            presentation_digest: self
                .presentation
                .as_ref()
                .map(|presentation| presentation.digest.as_str()),
        })
        .map_err(|_| CapsuleError::Malformed)?;
        Ok(digest(&bytes))
    }
}

impl InlineDocument {
    pub fn new(media_type: &str, text: String, sensitivity: &str) -> Self {
        Self {
            media_type: media_type.to_owned(),
            digest: digest(text.as_bytes()),
            text,
            sensitivity: sensitivity.to_owned(),
        }
    }
}

fn validate_inline(
    document: Option<&InlineDocument>,
    media_type: &str,
) -> Result<(), CapsuleError> {
    let Some(document) = document else {
        return Ok(());
    };
    if document.media_type != media_type
        || document.text.len() > MAXIMUM_AUXILIARY_BYTES
        || document.digest != digest(document.text.as_bytes())
        || !matches!(document.sensitivity.as_str(), "public" | "restricted")
        || serde_json::from_str::<serde_json::Value>(&document.text).is_err()
    {
        return Err(CapsuleError::InvalidAuxiliaryDocument);
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapsuleError {
    UnsupportedVersion,
    LimitExceeded,
    InvalidSource,
    InvalidAuxiliaryDocument,
    InvalidReference,
    IdentityMismatch,
    Malformed,
}

impl CapsuleError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-CAP-001",
            Self::LimitExceeded => "CND-CAP-002",
            Self::InvalidSource => "CND-CAP-003",
            Self::InvalidAuxiliaryDocument => "CND-CAP-004",
            Self::InvalidReference => "CND-CAP-005",
            Self::IdentityMismatch => "CND-CAP-006",
            Self::Malformed => "CND-CAP-007",
        }
    }
}

impl fmt::Display for CapsuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for CapsuleError {}
