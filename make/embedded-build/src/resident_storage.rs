//! Explicit physical realization choices sealed into resident artifacts.
/// Physical execution storage selected by fabrication, never execution authority.
/// SharedWorkspace requires the target's `execution-workspace` feature and a
/// separately funded scheduler workspace before any Step may run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidentExecutionStorage {
    Owned,
    SharedWorkspace,
    /// Shared step workspace with explicitly linked schema/node value storage.
    /// Requires both target features; exact routing and pool admission remain
    /// independent prerequisites, and canonical boundary traffic is preserved.
    SharedWorkspaceSchema,
}
impl ResidentExecutionStorage {
    pub(crate) fn uses_workspace(self) -> bool {
        matches!(self, Self::SharedWorkspace | Self::SharedWorkspaceSchema)
    }
}
