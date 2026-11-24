use std::fs;

use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, HostImage,
};

pub(crate) fn resolve_profile(
    profile_path: &std::path::Path,
    source_identity: String,
) -> Result<(HostImage, Vec<u8>), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(profile_path)?;
    if !profile_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".host.conduit"))
    {
        return Err(format!(
            "Host construction source must use the canonical .host.conduit suffix: {}",
            profile_path.display()
        )
        .into());
    }
    let configuration = parse_host_configuration_conduit(&source)
        .map_err(|diagnostic| format!("Host configuration decode refused: {diagnostic:?}"))?;
    let profile = check_host_configuration(
        configuration,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .map_err(|diagnostics| format!("Host configuration refused: {diagnostics:?}"))?
    .into_profile();
    let inputs = BuildInputs {
        source_identity,
        toolchain_available: true,
    };
    build_default_host_image(
        profile,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &inputs,
    )
    .map_err(|diagnostics| format!("Host BUILD refused: {diagnostics:?}").into())
}
