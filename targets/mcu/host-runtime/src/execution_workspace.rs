//! Fund the same prepared kernel before the whole-workset admission boundary.
use super::*;
use conduit_kernel::scheduler::PreparedStepWorkspace;
#[derive(Debug, PartialEq, Eq)]
pub enum WorkspaceFundingRefusal {
    Uninspectable,
    Requirements,
    Overflow,
    Capacity { required: usize, available: usize },
    AlreadyFunded,
}
impl<
        B: PreparedBacks<N, P>,
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const S: usize,
        L: ExecutionSigns,
        const K: usize,
    > PreparedFragment<B, N, C, P, R, T, H, S, L, K>
{
    /// Preserve every original owner on refusal. The original preparation
    /// ceiling remains fixed; neither this inspection nor physical
    /// storage acquisition admits resources, withdraws Backs or Starts a Play.
    /// The Option's inline bytes are already in kernel storage; charge only
    /// the additional actual Vec backing, exactly once.
    #[allow(clippy::result_large_err)] // Do not allocate to return an affine owner.
    pub fn fund_execution_workspace(
        &mut self,
        workspace: PreparedStepWorkspace,
    ) -> Result<(), (WorkspaceFundingRefusal, PreparedStepWorkspace)> {
        use WorkspaceFundingRefusal as E;
        let Some(requirements) = self.backs.execution_workspace_requirements() else {
            return Err((E::Uninspectable, workspace));
        };
        if requirements
            .iter()
            .flatten()
            .any(|r| !workspace.supports(*r))
        {
            return Err((E::Requirements, workspace));
        }
        let backing = workspace.reserved_backing_bytes();
        let Some(required) = self.retained.checked_add(backing) else {
            return Err((E::Overflow, workspace));
        };
        if required > self.maximum_retained_bytes {
            return Err((
                E::Capacity {
                    required,
                    available: self.maximum_retained_bytes,
                },
                workspace,
            ));
        }
        self.storage
            .install_execution_workspace(workspace, backing)
            .map_err(|(_, owner)| (E::AlreadyFunded, owner))?;
        self.retained = required;
        Ok(())
    }
}
#[cfg(test)]
#[path = "execution_workspace_tests.rs"]
mod tests;
