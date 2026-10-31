use super::{ConfigurationDiagnostic, HostConfiguration};

// Configuration identity predates the canonical Conduit construction source.
// Keep this encoding private so accepted identities remain stable without
// retaining TOML as a parseable or authorable construction format.
pub(super) fn stable_host_configuration_identity_source(
    configuration: &HostConfiguration,
) -> Result<String, ConfigurationDiagnostic> {
    let mut canonical = configuration.clone();
    // This is the checked realization identity, not a full-document digest.
    // Descriptive placement cannot invalidate firmware/equipment pins.
    canonical.metadata = None;
    canonical
        .bases
        .sort_by(|left, right| left.kind.cmp(&right.kind));
    canonical
        .resources
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical.buses.sort_by(|a, b| a.id().cmp(b.id()));
    canonical.devices.sort_by(|a, b| a.id().cmp(b.id()));
    if let Some(software) = &mut canonical.software {
        software.canonicalize();
    }
    // Attachment configurations have a distinct deterministic identity domain:
    // their generic unsigned parameters include u64 values outside TOML's i64.
    // Empty legacy configurations retain their original byte encoding below.
    if !canonical.devices.is_empty() {
        return serde_json::to_string(&canonical)
            .map(|json| format!("conduit.host.attachments@1\n{json}"))
            .map_err(|error| ConfigurationDiagnostic::Encode {
                detail: error.to_string(),
            });
    }
    toml::to_string_pretty(&canonical).map_err(|error| ConfigurationDiagnostic::Encode {
        detail: error.to_string(),
    })
}
