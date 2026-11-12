//! Checked native immutable Text residence selection. Filesystem effects belong to Boot.
use conduit_host_make::CheckedHostConfiguration;
use conduit_workspace_make::CheckedTextFileInitialization;
pub(super) fn parameters(
    checked: &CheckedHostConfiguration,
) -> Result<Option<CheckedTextFileInitialization>, String> {
    if !checked
        .configuration()
        .bases
        .iter()
        .any(|base| base.kind == conduit_std_offers::DURABLE_TEXT_BASE_KIND)
    {
        return Ok(None);
    }
    CheckedTextFileInitialization::from_configuration(checked).map(Some)
}
