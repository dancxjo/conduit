use crate::{ConfigurationDiagnostic, HostConfiguration};
use sha2::{Digest, Sha256};

/// Descriptive identity of this Host construction using the existing stable
/// declaration encoding. This hashes data only: success proves neither catalog
/// checking, native initialization, carrier custody, resource admission nor any
/// runtime authority. Callers must independently validate the original source.
/// The encoding remains private; this is not an alternative source format.
pub fn host_configuration_identity(
    configuration: &HostConfiguration,
) -> Result<String, ConfigurationDiagnostic> {
    let canonical = stable_host_configuration_identity_source(configuration)?;
    Ok(format!("sha256:{:x}", Sha256::digest(canonical.as_bytes())))
}

// Configuration identity predates the canonical Conduit construction source.
// Keep this encoding private so accepted identities remain stable without
// retaining TOML as a parseable or authorable construction format.
pub(super) fn stable_host_configuration_identity_source(
    configuration: &HostConfiguration,
) -> Result<String, ConfigurationDiagnostic> {
    let mut canonical = configuration.clone();
    canonical.normalize_declarations();
    toml::to_string_pretty(&canonical).map_err(|error| ConfigurationDiagnostic::Encode {
        detail: error.to_string(),
    })
}

impl HostConfiguration {
    // Outer declarations are sets keyed by kind/id. Preference lists inside a
    // declaration remain ordered. Use the same representation for checking,
    // source rendering and the existing stable identity encoding.
    pub(crate) fn normalize_declarations(&mut self) {
        self.bases.sort_by(|left, right| left.kind.cmp(&right.kind));
        self.resources.sort_by(|left, right| left.id.cmp(&right.id));
        self.capabilities.sort_by(|a, b| a.kind.cmp(&b.kind));
    }
}

#[cfg(test)]
mod tests;
