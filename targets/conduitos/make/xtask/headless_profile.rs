//! Resolved empty product selection, shared by build flags and proof admission.
use super::report::ArtifactRole;
use conduit_host_make::BuildManifest;
use std::process::Command;

pub(super) fn is_headless_product(manifest: &BuildManifest, role: ArtifactRole) -> bool {
    role == ArtifactRole::ProductHost
        && manifest.target == "conduitos/x86_64/pc"
        && manifest.presenters.is_empty()
        && manifest.facilities.is_empty()
        && manifest.resource_budgets.is_empty()
        && manifest.base_selections.is_empty()
        && manifest.driver_selections.is_empty()
        && manifest.host_calls.is_empty()
}

pub(super) fn configure_symbols(
    command: &mut Command,
    manifest: &BuildManifest,
    role: ArtifactRole,
) {
    if is_headless_product(manifest, role) {
        // The final product ELF is the proof input. Keep names inspectable without
        // debug data; graphical builds retain the ordinary release profile.
        command.env("CARGO_PROFILE_RELEASE_STRIP", "debuginfo");
    }
}

#[cfg(test)]
mod tests;
