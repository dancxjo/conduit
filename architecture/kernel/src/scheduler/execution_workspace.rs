//! Affine scheduler-owned execution buffers, loaned for exactly one Step.
//! Physical backing custody creates no semantic witness or execution grant.
use super::*;
use alloc::vec::Vec;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepWorkspaceRequirements {
    pub output_bytes: u32,
    pub scratch_bytes: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum WorkspacePreparationError {
    Empty,
    Overflow,
    Capacity { required: usize, available: usize },
    Allocation,
}
pub struct PreparedStepWorkspace {
    bytes: Vec<u8>,
    output_bytes: usize,
    scratch_bytes: usize,
}
impl core::fmt::Debug for PreparedStepWorkspace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PreparedStepWorkspace")
            .field("output_bytes", &self.output_bytes)
            .field("scratch_bytes", &self.scratch_bytes)
            .field("backing_bytes", &self.bytes.capacity())
            .finish()
    }
}
impl PreparedStepWorkspace {
    /// All allocation precedes execution. Root must separately charge this
    /// inline owner, actual buffer backing, allocator overhead and whole kernel.
    pub fn try_new(
        requirements: StepWorkspaceRequirements,
        maximum_backing: usize,
    ) -> Result<Self, WorkspacePreparationError> {
        use WorkspacePreparationError as E;
        let output_bytes = usize::try_from(requirements.output_bytes).map_err(|_| E::Overflow)?;
        let scratch_bytes = usize::try_from(requirements.scratch_bytes).map_err(|_| E::Overflow)?;
        let total = output_bytes.checked_add(scratch_bytes).ok_or(E::Overflow)?;
        if total == 0 {
            return Err(E::Empty);
        }
        if total > maximum_backing {
            return Err(E::Capacity {
                required: total,
                available: maximum_backing,
            });
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(total).map_err(|_| E::Allocation)?;
        if bytes.capacity() > maximum_backing {
            return Err(E::Capacity {
                required: bytes.capacity(),
                available: maximum_backing,
            });
        }
        bytes.resize(total, 0);
        Ok(Self {
            bytes,
            output_bytes,
            scratch_bytes,
        })
    }
    pub fn reserved_backing_bytes(&self) -> usize {
        self.bytes.capacity()
    }
    /// Physical extent inspection only; this grants no execution authority.
    pub fn supports(&self, r: StepWorkspaceRequirements) -> bool {
        usize::try_from(r.output_bytes).is_ok_and(|n| n <= self.output_bytes)
            && usize::try_from(r.scratch_bytes).is_ok_and(|n| n <= self.scratch_bytes)
    }
    pub(super) fn loan(
        &mut self,
        r: StepWorkspaceRequirements,
    ) -> Result<StepWorkspace<'_>, SchedulerError> {
        if !self.supports(r) {
            return Err(SchedulerError::InvalidPlan);
        }
        let (output, scratch) = self.bytes.split_at_mut(self.output_bytes);
        let output = &mut output[..r.output_bytes as usize];
        let scratch = &mut scratch[..r.scratch_bytes as usize];
        output.fill(0);
        scratch.fill(0);
        Ok(StepWorkspace { output, scratch })
    }
    pub(super) fn output(&self, len: u32) -> Result<&[u8], SchedulerError> {
        let len = usize::try_from(len).map_err(|_| SchedulerError::InvalidPlan)?;
        self.bytes
            .get(..len)
            .filter(|_| len <= self.output_bytes)
            .ok_or(SchedulerError::InvalidPlan)
    }
}
/// Safe Rust cannot retain these exclusive borrows after the synchronous Step.
/// Only the requested slices are exposed; no other Back can borrow meanwhile.
pub struct StepWorkspace<'a> {
    output: &'a mut [u8],
    scratch: &'a mut [u8],
}
impl StepWorkspace<'_> {
    pub fn buffers(&mut self) -> (&mut [u8], &mut [u8]) {
        (self.output, self.scratch)
    }
}
impl<const P: usize> StepIo<P> {
    /// Stage this Step's loaned output. Publication still requires the ordinary
    /// envelope, pressure, fuel, value-store and transaction checks.
    pub fn send_workspace_output(
        &mut self,
        port: PortId,
        length: u32,
    ) -> Result<(), SchedulerError> {
        if self.workspace_output_capacity.is_none_or(|n| length > n)
            || self
                .prepared_outputs
                .iter()
                .any(|v| matches!(v, Some(PreparedStepValue::Workspace(_))))
        {
            return self.fail(SchedulerError::OutputBlocked);
        }
        self.send_prepared(port, length)?;
        self.prepared_outputs[usize::from(port.0)] = Some(PreparedStepValue::Workspace(length));
        Ok(())
    }
}
impl<
        D,
        S,
        E,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const A: usize,
        const TERMINAL_ROWS: usize,
        const PHASE_BYTES: usize,
    > FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A, TERMINAL_ROWS, PHASE_BYTES>
where
    D: StepBack<P>,
    S: ValueStorage,
    E: SignSink,
{
    /// Install original owned backing before any execution. Refusal returns
    /// that same owner, and never allocates, enlarges or replaces a grant.
    // Returning the original affine owner on refusal must not allocate a box.
    #[allow(clippy::result_large_err)]
    pub fn install_execution_workspace(
        &mut self,
        workspace: PreparedStepWorkspace,
    ) -> Result<(), (SchedulerError, PreparedStepWorkspace)> {
        if self.decisions != 0
            || self.logical_invocations != 0
            || self.cancelled
            || self.execution_workspace.is_some()
            || self.drivers[..self.active_nodes]
                .iter()
                .filter_map(|d| d.workspace_requirements())
                .any(|r| !workspace.supports(r))
        {
            return Err((SchedulerError::InvalidPlan, workspace));
        }
        self.execution_workspace = Some(workspace);
        self.unmet_execution_workspace = false;
        Ok(())
    }
    pub fn execution_workspace_backing_bytes(&self) -> usize {
        self.execution_workspace
            .as_ref()
            .map_or(0, PreparedStepWorkspace::reserved_backing_bytes)
    }
}

impl<
        D,
        S,
        E,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const A: usize,
        const TERMINAL_ROWS: usize,
        const PHASE_BYTES: usize,
    >
    PreparedKernelStorage<
        D,
        FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A, TERMINAL_ROWS, PHASE_BYTES>,
        N,
    >
where
    D: StepBack<P>,
    S: ValueStorage,
    E: SignSink,
{
    /// Transfer funded physical backing before any driver is withdrawn. This
    /// supplies no admission or Ready evidence. Root must independently check
    /// exact selected-owner requirements and charge the complete retained
    /// workset; the ceiling here covers this Vec's actual backing only.
    /// Binding checks every actual active Back once. Missing/undersized backing
    /// then refuses before any Step, without allocating during binding.
    #[allow(clippy::result_large_err)] // Refusal returns the original affine owner.
    pub fn install_execution_workspace(
        &mut self,
        workspace: PreparedStepWorkspace,
        maximum_backing_bytes: usize,
    ) -> Result<(), (SchedulerError, PreparedStepWorkspace)> {
        if workspace.reserved_backing_bytes() > maximum_backing_bytes {
            return Err((SchedulerError::InvalidPlan, workspace));
        }
        // SAFETY: private construction initializes this exact field before
        // exposing PreparedKernelStorage. Drivers remain uninitialized: form
        // only a reference to the already initialized Option, never to K.
        let slot =
            unsafe { &mut *core::ptr::addr_of_mut!((*self.unbound_pointer()).execution_workspace) };
        if slot.is_some() {
            return Err((SchedulerError::InvalidPlan, workspace));
        }
        *slot = Some(workspace);
        Ok(())
    }
}
#[cfg(test)]
mod tests;
