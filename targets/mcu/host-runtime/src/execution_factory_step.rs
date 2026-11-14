//! Exact driver dispatch preserves each owner's schema, workspace and Step laws.
use super::*;

/// Uninhabited default keeps existing Driver inventories free of extra state.
pub enum NoExtraBack {}
impl<const PORTS: usize> StepBack<PORTS> for NoExtraBack {
    fn step(&mut self, _: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match *self {}
    }
}
pub enum Driver<'a, R, X = NoExtraBack> {
    Resident(R),
    Software(SoftwareBack<'a>),
    Native(HostCallBack),
    Write(WriteBack),
    Extra(X),
}
impl<R: StepBack<P>, X: StepBack<P>, const P: usize> StepBack<P> for Driver<'_, R, X> {
    #[cfg(feature = "execution-workspace")]
    fn workspace_requirements(
        &self,
    ) -> Option<conduit_kernel::scheduler::StepWorkspaceRequirements> {
        match self {
            Self::Resident(back) => back.workspace_requirements(),
            Self::Software(back) => <SoftwareBack as StepBack<P>>::workspace_requirements(back),
            Self::Extra(back) => back.workspace_requirements(),
            _ => None,
        }
    }
    #[cfg(feature = "execution-workspace")]
    fn step_with_workspace(
        &mut self,
        io: &mut StepIo<P>,
        input: &StepInputBytes<'_, P>,
        workspace: &mut conduit_kernel::scheduler::StepWorkspace<'_>,
    ) -> StepOutcome {
        match self {
            Self::Resident(back) => back.step_with_workspace(io, input, workspace),
            Self::Software(back) => {
                <SoftwareBack as StepBack<P>>::step_with_workspace(back, io, input, workspace)
            }
            Self::Extra(back) => back.step_with_workspace(io, input, workspace),
            _ => self.step(io, input),
        }
    }
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Resident(back) => back.canonical_input_schema(port),
            Self::Software(back) => {
                <SoftwareBack as StepBack<P>>::canonical_input_schema(back, port)
            }
            Self::Extra(back) => back.canonical_input_schema(port),
            _ => None,
        }
    }
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Resident(back) => back.canonical_output_schema(port),
            Self::Software(back) => {
                <SoftwareBack as StepBack<P>>::canonical_output_schema(back, port)
            }
            Self::Extra(back) => back.canonical_output_schema(port),
            _ => None,
        }
    }

    fn terminal_transductions(
        &self,
    ) -> [Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P] {
        match self {
            Self::Resident(back) => back.terminal_transductions(),
            Self::Software(back) => <SoftwareBack as StepBack<P>>::terminal_transductions(back),
            Self::Extra(back) => back.terminal_transductions(),
            _ => [None; P],
        }
    }
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        match self {
            Self::Resident(back) => back.step(io, input),
            Self::Software(back) => back.step(io, input),
            Self::Native(back) => back.step(io, input),
            Self::Write(back) => back.step(io, input),
            Self::Extra(back) => back.step(io, input),
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Resident(back) => back.prepared_output(port),
            Self::Software(back) => <SoftwareBack as StepBack<P>>::prepared_output(back, port),
            Self::Native(back) => <HostCallBack as StepBack<P>>::prepared_output(back, port),
            Self::Write(back) => <WriteBack as StepBack<P>>::prepared_output(back, port),
            Self::Extra(back) => back.prepared_output(port),
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Resident(back) => back.step_committed(),
            Self::Software(back) => <SoftwareBack as StepBack<P>>::step_committed(back),
            Self::Native(back) => <HostCallBack as StepBack<P>>::step_committed(back),
            Self::Write(back) => <WriteBack as StepBack<P>>::step_committed(back),
            Self::Extra(back) => back.step_committed(),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Resident(back) => back.cancel(),
            Self::Software(back) => <SoftwareBack as StepBack<P>>::cancel(back),
            Self::Native(back) => <HostCallBack as StepBack<P>>::cancel(back),
            Self::Write(back) => <WriteBack as StepBack<P>>::cancel(back),
            Self::Extra(back) => back.cancel(),
        }
    }
}
