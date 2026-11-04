use crate::{ConfigurationDiagnostic, HostConfiguration};

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
