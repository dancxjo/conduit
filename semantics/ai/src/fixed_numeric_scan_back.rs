//! Explicit one-pole scan with one atomic value and next-state proposal.
use crate::{
    fixed_neural::fixed_one_pole,
    fixed_numeric_catalog::{fixed_numeric_contracts, fixed_numeric_type},
    fixed_numeric_codec::{FixedCodecRefusal, FixedF32VectorCodec},
    fixed_numeric_preparation::{AFFINE_PROFILE, FixedPlannedRefusal, verify_fixed_placement},
};
use alloc::{string::String, vec};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, PortId,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
pub const SCAN_IMPLEMENTATION: &str = "conduit.numeric/scalar-explicit-one-pole@1";
pub fn fixed_one_pole_offer() -> Result<CapabilityOffer, String> {
    let kind = fixed_numeric_contracts()?
        .into_iter()
        .find(|k| k.kind_id.as_str() == "numeric/one-pole40")
        .ok_or_else(|| String::from("absent exact one-pole40 contract"))?;
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(SCAN_IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(AFFINE_PROFILE),
            implementation_id: ImplementationId::from(SCAN_IMPLEMENTATION),
            artifact_id: ArtifactId::from(SCAN_IMPLEMENTATION),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
pub const FLOW_SCAN_IMPLEMENTATION: &str = "conduit.numeric/closing-flow-one-pole40@1";
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedScanPreparationRefusal {
    Planned(FixedPlannedRefusal),
    Codec(FixedCodecRefusal),
    StepBudget,
}
pub struct FixedOnePoleBack {
    input: FixedF32VectorCodec<40>,
    scalar: FixedF32VectorCodec<1>,
    output: FixedF32VectorCodec<41>,
    staged: bool,
    finished: bool,
    cancelled: bool,
    flow: bool,
    committed_frames: u64,
}
impl FixedOnePoleBack {
    pub fn prepare_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
    ) -> Result<Self, FixedScanPreparationRefusal> {
        Self::prepare_internal::<PORTS>(placement, fuel, false)
    }
    pub fn prepare_flow_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
    ) -> Result<Self, FixedScanPreparationRefusal> {
        Self::prepare_internal::<PORTS>(placement, fuel, true)
    }
    fn prepare_internal<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        flow: bool,
    ) -> Result<Self, FixedScanPreparationRefusal> {
        if PORTS < 3 || fuel < 4 {
            return Err(FixedScanPreparationRefusal::StepBudget);
        }
        let offer = (if flow {
            crate::fixed_numeric_temporal::closing_numeric_offer(
                "numeric/one-pole40",
                FLOW_SCAN_IMPLEMENTATION,
            )
        } else {
            fixed_one_pole_offer()
        })
        .map_err(|_| FixedScanPreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape))?;
        verify_fixed_placement(placement, &offer).map_err(FixedScanPreparationRefusal::Planned)?;
        let ty = |name| {
            fixed_numeric_type(name)
                .map_err(|_| FixedScanPreparationRefusal::Codec(FixedCodecRefusal::Shape))
        };
        Ok(Self {
            input: FixedF32VectorCodec::prepare(&ty("NumericF32Vector40")?)
                .map_err(FixedScanPreparationRefusal::Codec)?,
            scalar: FixedF32VectorCodec::prepare(&ty("NumericF32Vector1")?)
                .map_err(FixedScanPreparationRefusal::Codec)?,
            output: FixedF32VectorCodec::prepare_one_pole_result()
                .map_err(FixedScanPreparationRefusal::Codec)?,
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
impl<const PORTS: usize> StepBack<PORTS> for FixedOnePoleBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 1700,
            });
        }
        self.staged = false;
        if self.committed_frames == u64::MAX {
            return fail(1701);
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if PORTS < 3 {
            return fail(1701);
        }
        if (0..3).any(|p| io.input_closed(PortId(p))) {
            if !self.flow {
                return fail(1701);
            }
            if (0..3).all(|p| io.input_closed(PortId(p))) {
                return StepOutcome::Complete;
            }
            if (0..3).any(|p| io.input(PortId(p)).is_some()) {
                return fail(1751);
            }
            return StepOutcome::Await;
        }
        if (0..3).any(|p| io.input(PortId(p)).is_none()) || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (Some(value), Some(coefficient), Some(prior)) = (
            inputs.input(PortId(0)),
            inputs.input(PortId(1)),
            inputs.input(PortId(2)),
        ) else {
            return fail(1702);
        };
        let mut input = [0.; 40];
        let mut coefficient_value = [0.];
        let mut prior_value = [0.];
        if self.input.decode(value, &mut input).is_err()
            || self
                .scalar
                .decode(coefficient, &mut coefficient_value)
                .is_err()
            || self.scalar.decode(prior, &mut prior_value).is_err()
        {
            return fail(1703);
        }
        let mut output = [0.; 40];
        let mut next = 0.;
        if fixed_one_pole(
            &input,
            coefficient_value[0],
            prior_value[0],
            &mut output,
            &mut next,
        )
        .is_err()
        {
            return fail(1704);
        }
        let mut combined = [0.; 41];
        combined[0] = next;
        combined[1..].copy_from_slice(&output);
        if self.output.encode(&combined).is_err() {
            return fail(1705);
        }
        for p in 0..3 {
            if io.consume(PortId(p)).is_err() {
                return fail(1706);
            }
        }
        if io
            .send_prepared(PortId(0), self.output.encoded().len() as u32)
            .is_err()
        {
            return fail(1707);
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
