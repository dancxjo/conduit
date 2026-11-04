//! Shared checked Host construction source entrance.
use conduit_host_make::{check_host_configuration, parse_host_configuration_conduit};

pub(crate) fn load_configuration(
    path: &std::path::Path,
) -> Result<conduit_host_make::CheckedHostConfiguration, Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".host.conduit"))
    {
        return Err(format!(
            "Host construction source must use the canonical .host.conduit suffix: {}",
            path.display()
        )
        .into());
    }
    let configuration = parse_host_configuration_conduit(&source)
        .map_err(|diagnostic| format!("Host configuration decode refused: {diagnostic:?}"))?;
    check_host_configuration(
        configuration,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .map_err(|diagnostics| format!("Host configuration refused: {diagnostics:?}").into())
}
