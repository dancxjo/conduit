use super::{PortTemporal, Stage, StageSink, StageSource};

pub(super) fn output_temporal(stage: &Stage) -> Option<PortTemporal> {
    match &stage.output {
        Some(StageSource::Internal(endpoint)) => Some(endpoint.port.temporal),
        Some(StageSource::FaceInput(_, _, temporal, _, _)) => Some(*temporal),
        None => None,
    }
}

pub(super) fn input_temporal(stage: &Stage) -> Option<PortTemporal> {
    let inputs = stage.input.as_ref()?;
    let first = inputs.first().map(|sink| match sink {
        StageSink::Internal(endpoint) => endpoint.port.temporal,
        StageSink::FaceOutput(_, _, temporal, _, _) => *temporal,
    })?;
    inputs
        .iter()
        .all(|sink| match sink {
            StageSink::Internal(endpoint) => endpoint.port.temporal == first,
            StageSink::FaceOutput(_, _, temporal, _, _) => *temporal == first,
        })
        .then_some(first)
}
