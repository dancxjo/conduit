//! Distinct native adapter terminal and admission refusals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayRefusal {
    Preparation,
    Protection(crate::composition::MachineRunError),
    Kernel,
    Scheduler(conduit_kernel::scheduler::SchedulerError),
    HostFailure(conduit_kernel::Failure),
    Foreground,
    InputOwnership,
    InputPressure,
    WorkBound,
    EditorDraftExists,
    EditorSourceBound,
    Cancelled,
}
impl PlayRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Protection(error) => error.as_str(),
            Self::Preparation => "native-body-play-preparation-refused",
            Self::Kernel => "native-body-kernel-boundary-refused",
            Self::Scheduler(_) => "native-body-kernel-step-refused",
            Self::HostFailure(failure) => failure.code.as_str(),
            Self::Foreground => "native-body-foreground-unavailable",
            Self::InputOwnership => "native-body-input-ownership-unavailable",
            Self::InputPressure => "native-body-input-pressure",
            Self::WorkBound => "native-body-work-bound-exceeded",
            Self::EditorDraftExists => "native-editor-existing-draft",
            Self::EditorSourceBound => "native-editor-source-capacity",
            Self::Cancelled => "native-body-play-cancelled",
        }
    }
}
