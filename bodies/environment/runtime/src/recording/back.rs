//! Recorder driver composition beneath the one kernel. Snapshot effects remain
//! independently admitted Host Calls; this owns neither a store nor authority.
use super::{RecordingRecoveryBack, RecordingReferenceSource, RecordingSingletonBack};
use alloc::boxed::Box;
use conduit_kernel::{scheduler::*, PortId};

pub enum RecordingBack {
    Source(RecordingReferenceSource),
    Read(HostCallBack),
    Singleton(RecordingSingletonBack),
    Recovery(Box<RecordingRecoveryBack>),
    #[cfg(feature = "recording-kernel")]
    Concat(Box<conduit_composite::FlowConcatFiniteBack>),
}
impl<const P: usize> StepBack<P> for RecordingBack {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; P] {
        match self {
            #[cfg(feature = "recording-kernel")]
            Self::Concat(back) => {
                <conduit_composite::FlowConcatFiniteBack as StepBack<P>>::terminal_transductions(
                    back,
                )
            }
            _ => [None; P],
        }
    }
    fn step(&mut self, io: &mut StepIo<P>, bytes: &StepInputBytes<'_, P>) -> StepOutcome {
        match self {
            Self::Source(b) => b.step(io, bytes),
            Self::Read(b) => b.step(io, bytes),
            Self::Singleton(b) => b.step(io, bytes),
            Self::Recovery(b) => b.step(io, bytes),
            #[cfg(feature = "recording-kernel")]
            Self::Concat(b) => b.step(io, bytes),
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Source(b) => <RecordingReferenceSource as StepBack<P>>::prepared_output(b, port),
            #[cfg(feature = "recording-kernel")]
            Self::Concat(b) => {
                <conduit_composite::FlowConcatFiniteBack as StepBack<P>>::prepared_output(b, port)
            }
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source(b) => <RecordingReferenceSource as StepBack<P>>::step_committed(b),
            Self::Read(b) => <HostCallBack as StepBack<P>>::step_committed(b),
            Self::Singleton(b) => <RecordingSingletonBack as StepBack<P>>::step_committed(b),
            Self::Recovery(b) => <RecordingRecoveryBack as StepBack<P>>::step_committed(b),
            #[cfg(feature = "recording-kernel")]
            Self::Concat(b) => {
                <conduit_composite::FlowConcatFiniteBack as StepBack<P>>::step_committed(b)
            }
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Source(b) => <RecordingReferenceSource as StepBack<P>>::cancel(b),
            Self::Read(b) => <HostCallBack as StepBack<P>>::cancel(b),
            Self::Singleton(b) => <RecordingSingletonBack as StepBack<P>>::cancel(b),
            Self::Recovery(b) => <RecordingRecoveryBack as StepBack<P>>::cancel(b),
            #[cfg(feature = "recording-kernel")]
            Self::Concat(b) => <conduit_composite::FlowConcatFiniteBack as StepBack<P>>::cancel(b),
        }
    }
}
