//! Bounded authored trajectories; numerical policies are checked Source.
//! This allocating preparation/conformance seam is not a Play Back.
use crate::generated::*;
use crate::source_programs::*;
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use conduit_plot::PortableExpressionEvaluationRefusal;

#[derive(Debug)]
pub enum AudioTrajectoryRefusal {
    Admission(NativeBindingRefusal),
    InvalidProgram,
    Evaluation(PortableExpressionEvaluationRefusal),
    InvalidSegment,
    UnorderedOrOverlapping,
    MixedDomain,
    ForeignAnchor,
    MissingCoverage,
    InvalidOutput,
}

pub type AudioTrajectorySourceExecution = crate::AudioSourceExecution;
use crate::source_execution::{AudioSourceExecutionRefusal, Program};
impl From<AudioSourceExecutionRefusal> for AudioTrajectoryRefusal {
    fn from(value: AudioSourceExecutionRefusal) -> Self {
        match value {
            AudioSourceExecutionRefusal::Admission(v) => Self::Admission(v),
            AudioSourceExecutionRefusal::InvalidProgram => Self::InvalidProgram,
            AudioSourceExecutionRefusal::Evaluation(v) => Self::Evaluation(v),
            AudioSourceExecutionRefusal::InvalidOutput => Self::InvalidOutput,
        }
    }
}
struct SegmentProjection {
    left: AudioTrajectoryRatio,
    right: AudioTrajectoryRatio,
    step: bool,
}

pub struct PreparedAudioQuantityTrajectory {
    original: AudioQuantityTrajectory,
    original_canonical: Vec<u8>,
    projections: Vec<SegmentProjection>,
    preparation: Vec<AudioTrajectorySourceExecution>,
    anchor: Program,
    covers: Program,
    weight: Program,
    blend: Program,
    step_covers: Program,
    step_select: Program,
}

/// Original trajectory/query, selected segment and all exact Source executions.
pub struct AudioTrajectoryEvaluation {
    original: AudioQuantityTrajectory,
    original_canonical: Vec<u8>,
    query: AudioTrajectoryQuery,
    query_canonical: Vec<u8>,
    selected_segment: usize,
    selected_segment_canonical: Vec<u8>,
    executions: Vec<AudioTrajectorySourceExecution>,
    result: AudioTrajectoryQuantity,
    admitted_result_canonical: Vec<u8>,
}
impl AudioTrajectoryEvaluation {
    pub fn original(&self) -> &AudioQuantityTrajectory {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_canonical
    }
    pub fn query(&self) -> &AudioTrajectoryQuery {
        &self.query
    }
    pub fn query_canonical(&self) -> &[u8] {
        &self.query_canonical
    }
    pub fn selected_segment(&self) -> usize {
        self.selected_segment
    }
    pub fn selected_segment_canonical(&self) -> &[u8] {
        &self.selected_segment_canonical
    }
    pub fn executions(&self) -> &[AudioTrajectorySourceExecution] {
        &self.executions
    }
    pub fn result(&self) -> &AudioTrajectoryQuantity {
        &self.result
    }
    pub fn admitted_result_canonical(&self) -> &[u8] {
        &self.admitted_result_canonical
    }
}

impl PreparedAudioQuantityTrajectory {
    pub fn new(canonical: &[u8]) -> Result<Self, AudioTrajectoryRefusal> {
        let original = AudioQuantityTrajectory::decode(canonical)
            .map_err(AudioTrajectoryRefusal::Admission)?;
        let ratio = Program::new(TRAJECTORY_RATIO)?;
        let valid = Program::new(TRAJECTORY_VALID)?;
        let order = Program::new(TRAJECTORY_ORDER_U32)?;
        let is_step = Program::new(TRAJECTORY_IS_STEP)?;
        let step_valid = Program::new(TRAJECTORY_STEP_VALID)?;
        let domain = Program::new(TRAJECTORY_DOMAIN)?;
        let mut preparation = Vec::new();
        let mut projections: Vec<SegmentProjection> = Vec::with_capacity(16);
        for (index, segment) in original.segments().as_slice().iter().enumerate() {
            let left: AudioTrajectoryRatio =
                ratio.native(segment.left().clone(), &mut preparation)?;
            let right: AudioTrajectoryRatio =
                ratio.native(segment.right().clone(), &mut preparation)?;
            let step = is_step.boolean(*segment.interpolation(), &mut preparation)?;
            let projection = SegmentProjection { left, right, step };
            let valid_segment = if step {
                step_valid.boolean(
                    step_eligibility(segment, &projection, segment.start(), false)?,
                    &mut preparation,
                )?
            } else {
                valid.boolean(
                    eligibility(segment, &projection, segment.start(), false)?,
                    &mut preparation,
                )?
            };
            if !valid_segment {
                return Err(AudioTrajectoryRefusal::InvalidSegment);
            }
            if let Some(first) = projections.first() {
                let compared = AudioTrajectoryDomainComparison::new(
                    *first.left.domain(),
                    *projection.left.domain(),
                )
                .map_err(AudioTrajectoryRefusal::Admission)?;
                if !domain.boolean(compared, &mut preparation)? {
                    return Err(AudioTrajectoryRefusal::MixedDomain);
                }
                let previous = &original.segments().as_slice()[index - 1];
                let pair = AudioTrajectorySegmentOrder::new(
                    segment.start().clone(),
                    previous.end().clone(),
                )
                .map_err(AudioTrajectoryRefusal::Admission)?;
                let eligible = AudioTrajectoryOrderU32::new(pair)
                    .map_err(AudioTrajectoryRefusal::Admission)?;
                if !order.boolean(eligible, &mut preparation)? {
                    return Err(AudioTrajectoryRefusal::UnorderedOrOverlapping);
                }
            }
            projections.push(projection);
        }
        Ok(Self {
            original,
            original_canonical: canonical.into(),
            projections,
            preparation,
            anchor: Program::new(TRAJECTORY_ANCHOR)?,
            covers: Program::new(TRAJECTORY_COVERS)?,
            weight: Program::new(TRAJECTORY_WEIGHT)?,
            blend: Program::new(TRAJECTORY_BLEND)?,
            step_covers: Program::new(TRAJECTORY_STEP_COVERS)?,
            step_select: Program::new(TRAJECTORY_STEP_SELECT)?,
        })
    }

    pub fn evaluate(
        &self,
        canonical: &[u8],
    ) -> Result<AudioTrajectoryEvaluation, AudioTrajectoryRefusal> {
        let query =
            AudioTrajectoryQuery::decode(canonical).map_err(AudioTrajectoryRefusal::Admission)?;
        let mut executions = self.preparation.clone();
        let comparison = AudioTrajectoryAnchorComparison::new(
            query.anchor().clone(),
            self.original.anchor().clone(),
        )
        .map_err(AudioTrajectoryRefusal::Admission)?;
        if !self.anchor.boolean(comparison, &mut executions)? {
            return Err(AudioTrajectoryRefusal::ForeignAnchor);
        }
        // Source decides coverage; Rust only traverses the admitted finite sequence.
        for (index, segment) in self.original.segments().as_slice().iter().enumerate() {
            let raw: AudioTrajectoryRatio = if self.projections[index].step {
                let eligible = step_eligibility(
                    segment,
                    &self.projections[index],
                    query.time(),
                    index + 1 == self.projections.len(),
                )?;
                if !self
                    .step_covers
                    .boolean(eligible.clone(), &mut executions)?
                {
                    continue;
                }
                self.step_select.native(eligible, &mut executions)?
            } else {
                let eligible = eligibility(
                    segment,
                    &self.projections[index],
                    query.time(),
                    index + 1 == self.projections.len(),
                )?;
                if !self.covers.boolean(eligible.clone(), &mut executions)? {
                    continue;
                }
                let weight: AudioTrajectoryWeight =
                    self.weight.native(eligible.clone(), &mut executions)?;
                let blend = AudioTrajectoryBlendInput::new(eligible, weight)
                    .map_err(AudioTrajectoryRefusal::Admission)?;
                self.blend.native(blend, &mut executions)?
            };
            let result = admit_result(raw)?;
            let admitted_result_canonical = result
                .clone()
                .encode()
                .map_err(AudioTrajectoryRefusal::Admission)?;
            return Ok(AudioTrajectoryEvaluation {
                original: self.original.clone(),
                original_canonical: self.original_canonical.clone(),
                query,
                query_canonical: canonical.into(),
                selected_segment: index,
                selected_segment_canonical: segment
                    .clone()
                    .encode()
                    .map_err(AudioTrajectoryRefusal::Admission)?,
                executions,
                result,
                admitted_result_canonical,
            });
        }
        Err(AudioTrajectoryRefusal::MissingCoverage)
    }
}

fn eligibility(
    segment: &AudioTrajectorySegment,
    projection: &SegmentProjection,
    query: &AudioExactTimeOffset,
    final_segment: bool,
) -> Result<AudioTrajectoryU8Arithmetic, AudioTrajectoryRefusal> {
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
    AudioTrajectoryU8Arithmetic::new(input).map_err(AudioTrajectoryRefusal::Admission)
}

fn step_eligibility(
    segment: &AudioTrajectorySegment,
    projection: &SegmentProjection,
    query: &AudioExactTimeOffset,
    final_segment: bool,
) -> Result<AudioTrajectoryU32Step, AudioTrajectoryRefusal> {
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
    AudioTrajectoryU32Step::new(input).map_err(AudioTrajectoryRefusal::Admission)
}

// Exact field-copy admission; all interpolation arithmetic belongs to Source.
fn admit_result(
    raw: AudioTrajectoryRatio,
) -> Result<AudioTrajectoryQuantity, AudioTrajectoryRefusal> {
    let (n, d) = (*raw.numerator(), *raw.denominator());
    match raw.domain() {
        AudioTrajectoryDomain::Frequency => AudioTrajectoryQuantity::frequency(d, n),
        AudioTrajectoryDomain::Cycle => AudioTrajectoryQuantity::cycle(d, n),
        AudioTrajectoryDomain::Amplitude => AudioTrajectoryQuantity::amplitude(d, n),
        AudioTrajectoryDomain::Power => AudioTrajectoryQuantity::power(d, n),
    }
    .map_err(AudioTrajectoryRefusal::Admission)
}
