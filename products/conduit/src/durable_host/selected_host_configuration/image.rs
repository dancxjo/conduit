//! Recheck retained Source and the actual existing Make IMAGE before native Boot.
use super::*;
use conduit_host_make::{verify_configuration_image, verify_image_binding, HostImage};

pub(crate) fn check_seed_image(
    source: &str,
    image: &HostImage,
) -> Result<CheckedHostConfiguration, String> {
    let checked = check(source)?;
    let bytes = conduit_body_make::encode_joining_host_evidence(image, 64 * 1024)?;
    verify_image_binding(image, &bytes).map_err(|_| "Seed IMAGE payload identity changed")?;
    verify_configuration_image(&checked, image, &conduit_workspace_make::catalog())?;
    Ok(checked)
}
