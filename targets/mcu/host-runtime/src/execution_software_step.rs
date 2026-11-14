//! Ordinary kernel Step dispatch for the exact prepared software owner.
use super::*;
impl<const P: usize> StepBack<P> for SoftwareBack<'_> {
    #[cfg(feature = "execution-workspace")]
    fn workspace_requirements(
        &self,
    ) -> Option<conduit_kernel::scheduler::StepWorkspaceRequirements> {
        match self {
            Self::SeededState(back) => {
                <conduit_composite::SeededStateBack as StepBack<P>>::workspace_requirements(back)
            }
            Self::Merge(back) => {
                <conduit_composite::FlowMergeFiniteBack as StepBack<P>>::workspace_requirements(
                    back,
                )
            }
            Self::Zip(back) => {
                <conduit_composite::FlowZipBack as StepBack<P>>::workspace_requirements(back)
            }
            Self::WorkspaceSelector(back) => {
                <WorkspaceSelectorBack as StepBack<P>>::workspace_requirements(back)
            }
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
            Self::SeededState(back) => {
                <conduit_composite::SeededStateBack as StepBack<P>>::step_with_workspace(
                    back, io, input, workspace,
                )
            }
            Self::Merge(back) => {
                <conduit_composite::FlowMergeFiniteBack as StepBack<P>>::step_with_workspace(
                    back, io, input, workspace,
                )
            }
            Self::Zip(back) => {
                <conduit_composite::FlowZipBack as StepBack<P>>::step_with_workspace(
                    back, io, input, workspace,
                )
            }
            Self::WorkspaceSelector(back) => {
                <WorkspaceSelectorBack as StepBack<P>>::step_with_workspace(
                    back, io, input, workspace,
                )
            }
            _ => self.step(io, input),
        }
    }
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        self.input_schema::<P>(port)
    }
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        self.output_schema::<P>(port)
    }

    fn terminal_transductions(
        &self,
    ) -> [Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P] {
        match self {
            Self::SeededState(back) => {
                <conduit_composite::SeededStateBack as StepBack<P>>::terminal_transductions(back)
            }
            Self::Zip(back) => {
                <conduit_composite::FlowZipBack as StepBack<P>>::terminal_transductions(back)
            }
            Self::Merge(back) => {
                <conduit_composite::FlowMergeFiniteBack as StepBack<P>>::terminal_transductions(
                    back,
                )
            }
            _ => [None; P],
        }
    }

    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        match self {
            Self::LinkedSource(back) => {
                <conduit_typed_retention::PreparedLinkedSource as StepBack<P>>::step(
                    back, io, input,
                )
            }
            Self::SeededState(back) => {
                <conduit_composite::SeededStateBack as StepBack<P>>::step(back, io, input)
            }
            Self::Zip(back) => {
                <conduit_composite::FlowZipBack as StepBack<P>>::step(back, io, input)
            }
            Self::Merge(back) => {
                <conduit_composite::FlowMergeFiniteBack as StepBack<P>>::step(back, io, input)
            }
            Self::Tick(back) => back.step(io, input),
            Self::Trigger(back) => back.step(io, input),
            Self::Selector(back) => back.step(io, input),
            #[cfg(feature = "execution-workspace")]
            Self::WorkspaceSelector(back) => back.step(io, input),
            Self::Observation(back) => back.step(io, input),
            Self::RecordingLog(back) => back.step(io, input),
            Self::Publication(back) => back.step(io, input),
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::LinkedSource(back) => <conduit_typed_retention::PreparedLinkedSource as StepBack<P>>::prepared_output(back, port),
            Self::SeededState(back) => <conduit_composite::SeededStateBack as StepBack<P>>::prepared_output(back, port),
            Self::Zip(back) => <conduit_composite::FlowZipBack as StepBack<P>>::prepared_output(back, port),
            Self::Merge(back) => <conduit_composite::FlowMergeFiniteBack as StepBack<P>>::prepared_output(back, port),
            Self::Selector(back) => <SelectorBack as StepBack<P>>::prepared_output(back, port),
            #[cfg(feature = "execution-workspace")]
            Self::WorkspaceSelector(back)=><WorkspaceSelectorBack as StepBack<P>>::prepared_output(back,port),
            Self::Observation(back) => {
                <ObservationBack as StepBack<P>>::prepared_output(back, port)
            }
            Self::RecordingLog(back) => <conduit_environment_runtime::recording::RecordingLogBack as StepBack<P>>::prepared_output(back,port),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::LinkedSource(back) => <conduit_typed_retention::PreparedLinkedSource as StepBack<P>>::step_committed(back),
            Self::SeededState(back) => <conduit_composite::SeededStateBack as StepBack<P>>::step_committed(back),
            Self::Zip(back) => <conduit_composite::FlowZipBack as StepBack<P>>::step_committed(back),
            Self::Merge(back) => <conduit_composite::FlowMergeFiniteBack as StepBack<P>>::step_committed(back),
            Self::Tick(back) => <conduit_time::TickBack as StepBack<P>>::step_committed(back),
            Self::Trigger(back) => {
                <conduit_time::TickTriggerBack as StepBack<P>>::step_committed(back)
            }
            Self::Selector(back) => <SelectorBack as StepBack<P>>::step_committed(back),
            #[cfg(feature = "execution-workspace")]
            Self::WorkspaceSelector(back)=><WorkspaceSelectorBack as StepBack<P>>::step_committed(back),
            Self::Observation(back) => <ObservationBack as StepBack<P>>::step_committed(back),
            Self::RecordingLog(back) => <conduit_environment_runtime::recording::RecordingLogBack as StepBack<P>>::step_committed(back),
            Self::Publication(back) => <PublicationFaceBack as StepBack<P>>::step_committed(back),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::LinkedSource(back) => {
                <conduit_typed_retention::PreparedLinkedSource as StepBack<P>>::cancel(back)
            }
            Self::SeededState(back) => {
                <conduit_composite::SeededStateBack as StepBack<P>>::cancel(back)
            }
            Self::Zip(back) => <conduit_composite::FlowZipBack as StepBack<P>>::cancel(back),
            Self::Merge(back) => {
                <conduit_composite::FlowMergeFiniteBack as StepBack<P>>::cancel(back)
            }
            Self::Tick(back) => <conduit_time::TickBack as StepBack<P>>::cancel(back),
            Self::Trigger(back) => <conduit_time::TickTriggerBack as StepBack<P>>::cancel(back),
            Self::Selector(back) => <SelectorBack as StepBack<P>>::cancel(back),
            #[cfg(feature = "execution-workspace")]
            Self::WorkspaceSelector(back) => <WorkspaceSelectorBack as StepBack<P>>::cancel(back),
            Self::Observation(back) => <ObservationBack as StepBack<P>>::cancel(back),
            Self::RecordingLog(back) => {
                <conduit_environment_runtime::recording::RecordingLogBack as StepBack<P>>::cancel(
                    back,
                )
            }
            Self::Publication(back) => <PublicationFaceBack as StepBack<P>>::cancel(back),
        }
    }
}
