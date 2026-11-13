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
    Storage(conduit_kernel::scheduler::WorkspacePreparationError),
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
    /// Derive one pair of physical extents from the exact selected owners.
    /// All allocation is cold, fallible and bounded by the original remaining
    /// retained ceiling; this neither changes grants nor withdraws providers.
    pub fn prepare_execution_workspace(&mut self) -> Result<(), WorkspaceFundingRefusal> {
        use WorkspaceFundingRefusal as E;
        let reports = self
            .backs
            .execution_workspace_requirements()
            .ok_or(E::Uninspectable)?;
        let mut required = conduit_kernel::scheduler::StepWorkspaceRequirements {
            output_bytes: 0,
            scratch_bytes: 0,
        };
        let mut any = false;
        for report in reports.into_iter().flatten() {
            any = true;
            required.output_bytes = required.output_bytes.max(report.output_bytes);
            required.scratch_bytes = required.scratch_bytes.max(report.scratch_bytes);
        }
        if !any {
            return Ok(());
        }
        let remaining = self
            .maximum_retained_bytes
            .checked_sub(self.retained)
            .ok_or(E::Overflow)?;
        let workspace = PreparedStepWorkspace::try_new(required, remaining).map_err(E::Storage)?;
        self.fund_execution_workspace(workspace)
            .map_err(|(error, _owner)| error)
    }

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
