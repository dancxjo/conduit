//! Explicit bounded slicing/gathering without speech wrapping or pitch policy.
pub use crate::fixed_numeric_value_capacity::INDEX_IMPLEMENTATION;
use crate::{
    fixed_neural::{fixed_gather, fixed_slice},
    fixed_numeric_catalog::{fixed_numeric_contracts, fixed_numeric_type},
    fixed_numeric_codec::{FixedCodecRefusal, FixedF32VectorCodec},
    fixed_numeric_index_codec::FixedU16IndexCodec,
    fixed_numeric_preparation::{AFFINE_PROFILE, FixedPlannedRefusal, verify_fixed_placement},
};
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, PortId,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
pub const FLOW_INDEX_IMPLEMENTATION: &str = "conduit.numeric/closing-flow-index@1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixedIndexOperation {
    Slice,
    Gather,
}
impl FixedIndexOperation {
    fn name(self) -> &'static str {
        match self {
            Self::Slice => "slice",
            Self::Gather => "gather",
        }
    }
}
pub fn fixed_index_offer<const INPUT: usize, const OUTPUT: usize>(
    operation: FixedIndexOperation,
) -> Result<CapabilityOffer, String> {
    let name = format!("numeric/{}{INPUT}x{OUTPUT}", operation.name());
    let kind = fixed_numeric_contracts()?
        .into_iter()
        .find(|k| k.kind_id.as_str() == name)
        .ok_or_else(|| String::from("unsupported exact index operation"))?;
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(format!(
                "{INDEX_IMPLEMENTATION}/{}/{INPUT}x{OUTPUT}",
                operation.name()
            )),
            execution_profile_id: ExecutionProfileId::from(AFFINE_PROFILE),
            implementation_id: ImplementationId::from(INDEX_IMPLEMENTATION),
            artifact_id: ArtifactId::from(INDEX_IMPLEMENTATION),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedIndexPreparationRefusal {
    Planned(FixedPlannedRefusal),
    Codec(FixedCodecRefusal),
    StepBudget,
}
pub struct FixedIndexBack<const INPUT: usize, const OUTPUT: usize> {
    input: FixedF32VectorCodec<INPUT>,
    output: FixedF32VectorCodec<OUTPUT>,
    start: Option<FixedU16IndexCodec<1>>,
    indices: Option<FixedU16IndexCodec<OUTPUT>>,
    staged: bool,
    finished: bool,
    cancelled: bool,
    flow: bool,
    committed_frames: u64,
}
impl<const INPUT: usize, const OUTPUT: usize> FixedIndexBack<INPUT, OUTPUT> {
    pub fn prepare_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        operation: FixedIndexOperation,
    ) -> Result<Self, FixedIndexPreparationRefusal> {
        Self::prepare_internal::<PORTS>(placement, fuel, operation, false)
    }
    pub fn prepare_flow_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        operation: FixedIndexOperation,
    ) -> Result<Self, FixedIndexPreparationRefusal> {
        Self::prepare_internal::<PORTS>(placement, fuel, operation, true)
    }
    fn prepare_internal<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        operation: FixedIndexOperation,
        flow: bool,
    ) -> Result<Self, FixedIndexPreparationRefusal> {
        if PORTS < 2 || fuel < 3 {
            return Err(FixedIndexPreparationRefusal::StepBudget);
        }
        let offer = if flow {
            crate::fixed_numeric_temporal::closing_numeric_offer_for_placement(
                &format!("numeric/{}{INPUT}x{OUTPUT}", operation.name()),
                FLOW_INDEX_IMPLEMENTATION,
                placement,
            )
        } else {
            fixed_index_offer::<INPUT, OUTPUT>(operation).and_then(|base| {
                crate::fixed_numeric_value_capacity::value_offer_for_placement(base, placement)
            })
        }
        .map_err(|_| {
            FixedIndexPreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape)
        })?;
        verify_fixed_placement(placement, &offer).map_err(FixedIndexPreparationRefusal::Planned)?;
        let ty = |name: String| {
            fixed_numeric_type(&name)
                .map_err(|_| FixedIndexPreparationRefusal::Codec(FixedCodecRefusal::Shape))
        };
        let (start, indices) = match operation {
            FixedIndexOperation::Slice => (
                Some(
                    FixedU16IndexCodec::prepare(
                        &StructuredInfoType::leaf(kind_id("value/u16")).map_err(|_| {
                            FixedIndexPreparationRefusal::Codec(FixedCodecRefusal::Shape)
                        })?,
                    )
                    .map_err(FixedIndexPreparationRefusal::Codec)?,
                ),
                None,
            ),
            FixedIndexOperation::Gather => (
                None,
                Some(
                    FixedU16IndexCodec::prepare(&ty(format!("NumericU16Indices{OUTPUT}"))?)
                        .map_err(FixedIndexPreparationRefusal::Codec)?,
                ),
            ),
        };
        Ok(Self {
            input: FixedF32VectorCodec::prepare(&ty(format!("NumericF32Vector{INPUT}"))?)
                .map_err(FixedIndexPreparationRefusal::Codec)?,
            output: FixedF32VectorCodec::prepare(&ty(format!("NumericF32Vector{OUTPUT}"))?)
                .map_err(FixedIndexPreparationRefusal::Codec)?,
            start,
            indices,
            staged: false,
            finished: false,
            cancelled: false,
            flow,
            committed_frames: 0,
        })
    }
    pub fn committed_frames(&self) -> u64 {
        self.committed_frames
    }
    pub fn output(&self) -> &[u8] {
        self.output.encoded()
    }
    pub fn has_committed_result(&self) -> bool {
        self.finished
    }
}
impl<const INPUT: usize, const OUTPUT: usize, const PORTS: usize> StepBack<PORTS>
    for FixedIndexBack<INPUT, OUTPUT>
{
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 1600,
            });
        }
        self.staged = false;
        if self.committed_frames == u64::MAX {
            return fail(1599);
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if PORTS < 2 {
            return fail(1501);
        }
        if (0..2).any(|p| io.input_closed(PortId(p as u16))) {
            if !self.flow {
                return fail(1501);
            }
            if (0..2).all(|p| io.input_closed(PortId(p as u16))) {
                return StepOutcome::Complete;
            }
            if (0..2).any(|p| io.input(PortId(p as u16)).is_some()) {
                return fail(1598);
            }
            return StepOutcome::Await;
        }
        if (0..2).any(|p| io.input(PortId(p)).is_none()) || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (Some(value), Some(indices)) = (inputs.input(PortId(0)), inputs.input(PortId(1)))
        else {
            return fail(1602);
        };
        let mut input = [0.; INPUT];
        let mut output = [0.; OUTPUT];
        if self.input.decode(value, &mut input).is_err() {
            return fail(1603);
        }
        if let Some(codec) = &self.start {
            let mut index = [0];
            if codec.decode(indices, &mut index).is_err()
                || fixed_slice(&input, usize::from(index[0]), &mut output).is_err()
            {
                return fail(1604);
            }
        } else if let Some(codec) = &self.indices {
            let mut indices_value = [0; OUTPUT];
            if codec.decode(indices, &mut indices_value).is_err()
                || fixed_gather(&input, &indices_value.map(usize::from), &mut output).is_err()
            {
                return fail(1604);
            }
        } else {
            return fail(1604);
        }
        if self.output.encode(&output).is_err() {
            return fail(1605);
        }
        for p in 0..2 {
            if io.consume(PortId(p)).is_err() {
                return fail(1606);
            }
        }
        if io
            .send_prepared(PortId(0), self.output.encoded().len() as u32)
            .is_err()
        {
            return fail(1607);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.finished = !self.flow;
            self.committed_frames += 1;
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && (!self.flow || self.staged)).then(|| self.output.encoded())
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

impl<const INPUT: usize, const OUTPUT: usize> FixedIndexBack<INPUT, OUTPUT> {
    /// Retained requested local payload capacity. Shared tensor/model Arc owners,
    /// Box root, Arc headers, allocator bookkeeping and stack are separate charges.
    pub fn local_accounted_heap_bytes(&self) -> usize {
        0usize
            .saturating_add(self.input.local_accounted_heap_bytes())
            .saturating_add(self.output.local_accounted_heap_bytes())
            .saturating_add(
                self.start
                    .as_ref()
                    .map_or(0, |v| v.local_accounted_heap_bytes()),
            )
            .saturating_add(
                self.indices
                    .as_ref()
                    .map_or(0, |v| v.local_accounted_heap_bytes()),
            )
    }
}
