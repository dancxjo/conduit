//! Common-basis Source eligibility and execution; no numeric interpolation in Rust.
use super::*;

pub(super) struct CommonBasisLinear {
    valid: Program,
    covers: Program,
    interpolate: Program,
}
impl CommonBasisLinear {
    pub(super) fn new() -> Result<Self, AudioTrajectoryRefusal> {
        Ok(Self {
            valid: Program::new(TRAJECTORY_COMMON_BASIS_VALID)?,
            covers: Program::new(TRAJECTORY_COMMON_BASIS_COVERS)?,
            interpolate: Program::new(TRAJECTORY_COMMON_BASIS_INTERPOLATE)?,
        })
    }

    pub(super) fn valid(
        &self,
        segment: &AudioTrajectorySegment,
        projection: &SegmentProjection,
        shared_time_basis: &mut Option<AudioExactTimeOffset>,
        evidence: &mut Vec<AudioTrajectorySourceExecution>,
    ) -> Result<bool, AudioTrajectoryRefusal> {
        let eligible = eligibility(segment, projection, segment.start(), false)?;
        if let Some(previous) = shared_time_basis {
            AudioTrajectoryCommonBasisTimeMatch::new(previous.clone(), segment.start().clone())
                .map_err(AudioTrajectoryRefusal::Admission)?;
        } else {
            *shared_time_basis = Some(segment.start().clone());
        }
        self.valid.boolean(eligible, evidence).map_err(Into::into)
    }

    pub(super) fn evaluate(
        &self,
        segment: &AudioTrajectorySegment,
        projection: &SegmentProjection,
        query: &AudioExactTimeOffset,
        final_segment: bool,
        evidence: &mut Vec<AudioTrajectorySourceExecution>,
    ) -> Result<Option<AudioTrajectoryRatio>, AudioTrajectoryRefusal> {
        let eligible = eligibility(segment, projection, query, final_segment)?;
        if !self.covers.boolean(eligible.clone(), evidence)? {
            return Ok(None);
        }
        // Coverage is checked by Source; generated Native readmission confirms
        // subtraction and weighted-sum eligibility before interpolation runs.
        let covered = AudioTrajectoryCommonBasisCovered::new(eligible)
            .map_err(AudioTrajectoryRefusal::Admission)?;
        self.interpolate
            .native(covered, evidence)
            .map(Some)
            .map_err(Into::into)
    }
}

fn eligibility(
    segment: &AudioTrajectorySegment,
    projection: &SegmentProjection,
    query: &AudioExactTimeOffset,
    final_segment: bool,
) -> Result<AudioTrajectoryCommonBasisU32, AudioTrajectoryRefusal> {
    let input = AudioTrajectoryArithmeticInput::new(
        segment.end().clone(),
        final_segment,
        *segment.interpolation(),
        projection.left.clone(),
        query.clone(),
        projection.right.clone(),
        segment.start().clone(),
    )
    .map_err(AudioTrajectoryRefusal::Admission)?;
    AudioTrajectoryCommonBasisU32::new(input).map_err(AudioTrajectoryRefusal::Admission)
}
