use conduit_ai::{ModelComputeRefusal, TrainingRefusal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidDescriptor,
    ResourceBound,
    UnsupportedDevice,
    /// Burn 0.22 cannot restore optimizer state to this explicitly selected device.
    UnsupportedResumeDevice,
    InvalidTensor,
    IncompatibleCheckpoint,
    CorruptCheckpoint,
    CheckpointIo(String),
    /// Publication is visible but the final directory sync did not confirm durability.
    DurabilityUncertain([u8; 32]),
    NumericFailure,
    Cancelled,
    Unloaded,
    ModelCompute(ModelComputeRefusal),
    Training(TrainingRefusal),
}
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<ModelComputeRefusal> for Error {
    fn from(value: ModelComputeRefusal) -> Self {
        Self::ModelCompute(value)
    }
}
impl From<TrainingRefusal> for Error {
    fn from(value: TrainingRefusal) -> Self {
        Self::Training(value)
    }
}
