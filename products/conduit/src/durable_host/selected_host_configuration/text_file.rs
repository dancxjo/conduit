//! Checked native immutable Text residence selection. Filesystem effects belong to Boot.
use conduit_host_make::CheckedHostConfiguration;
use conduit_workspace_make::{CheckedTextFileInitialization, TEXT_FILE_BASE_KIND};
pub(super) fn parameters(
    checked: &CheckedHostConfiguration,
) -> Result<Option<CheckedTextFileInitialization>, String> {
    if !checked
        .configuration()
        .bases
        .iter()
        .any(|base| base.kind == TEXT_FILE_BASE_KIND)
    {
        return Ok(None);
    }
    CheckedTextFileInitialization::from_configuration(checked).map(Some)
}
