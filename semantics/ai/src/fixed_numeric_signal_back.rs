//! One-invocation finite elementwise operators. No model gate ordering is here.
pub use crate::fixed_numeric_value_capacity::ELEMENTWISE_IMPLEMENTATION;
use crate::{
    fixed_numeric_catalog::{fixed_numeric_contracts, fixed_numeric_type},
    fixed_numeric_codec::{FixedCodecRefusal, FixedF32VectorCodec},
    fixed_numeric_preparation::{verify_fixed_placement, FixedPlannedRefusal, AFFINE_PROFILE},
};
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub const FLOW_ELEMENTWISE_IMPLEMENTATION: &str = "conduit.numeric/closing-flow-elementwise@1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixedElementwiseOperation {
    Add,
    Multiply,
    Sigmoid,
    Complement,
    Exp,
    Scale,
    Clamp,
    ReciprocalOffset,
}
impl FixedElementwiseOperation {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Multiply => "multiply",
            Self::Sigmoid => "sigmoid",
            Self::Complement => "complement",
            Self::Exp => "exp",
            Self::Scale => "scale",
            Self::Clamp => "clamp",
            Self::ReciprocalOffset => "reciprocal-offset",
        }
    }
    fn arity(self) -> usize {
        match self {
            Self::Clamp => 3,
            Self::Add | Self::Multiply | Self::Scale | Self::ReciprocalOffset => 2,
            _ => 1,
        }
    }
    fn binary(self) -> bool {
        matches!(self, Self::Add | Self::Multiply)
    }
    fn scalars(self) -> usize {
        match self {
            Self::Clamp => 2,
            Self::Scale | Self::ReciprocalOffset => 1,
            _ => 0,
        }
    }
}
pub fn fixed_elementwise_offer<const WIDTH: usize>(
    operation: FixedElementwiseOperation,
) -> Result<CapabilityOffer, String> {
    let name = format!("numeric/{}{WIDTH}", operation.name());
    let kind = fixed_numeric_contracts()?
        .into_iter()
        .find(|k| k.kind_id.as_str() == name)
        .ok_or_else(|| String::from("unsupported exact elementwise contract"))?;
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(format!(
                "{ELEMENTWISE_IMPLEMENTATION}/{}/{WIDTH}",
                operation.name()
            )),
            execution_profile_id: ExecutionProfileId::from(AFFINE_PROFILE),
            implementation_id: ImplementationId::from(ELEMENTWISE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(ELEMENTWISE_IMPLEMENTATION),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedElementwisePreparationRefusal {
    Planned(FixedPlannedRefusal),
    Codec(FixedCodecRefusal),
    StepBudget,
}
pub struct FixedElementwiseBack<const WIDTH: usize> {
    operation: FixedElementwiseOperation,
    input: FixedF32VectorCodec<WIDTH>,
    right: Option<FixedF32VectorCodec<WIDTH>>,
    scalar: Option<FixedF32VectorCodec<1>>,
    output: FixedF32VectorCodec<WIDTH>,
    staged: bool,
    finished: bool,
    cancelled: bool,
    flow: bool,
    committed_frames: u64,
}
impl<const WIDTH: usize> FixedElementwiseBack<WIDTH> {
    pub fn prepare_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        operation: FixedElementwiseOperation,
    ) -> Result<Self, FixedElementwisePreparationRefusal> {
        Self::prepare_internal::<PORTS>(placement, fuel, operation, false)
    }
    pub fn prepare_flow_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        operation: FixedElementwiseOperation,
    ) -> Result<Self, FixedElementwisePreparationRefusal> {
        Self::prepare_internal::<PORTS>(placement, fuel, operation, true)
    }
    fn prepare_internal<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        operation: FixedElementwiseOperation,
        flow: bool,
    ) -> Result<Self, FixedElementwisePreparationRefusal> {
        if PORTS < operation.arity() || usize::from(fuel) < operation.arity() + 1 {
            return Err(FixedElementwisePreparationRefusal::StepBudget);
        }
        let offer = if flow {
            crate::fixed_numeric_temporal::closing_numeric_offer_for_placement(
                &format!("numeric/{}{WIDTH}", operation.name()),
                FLOW_ELEMENTWISE_IMPLEMENTATION,
                placement,
            )
        } else {
            fixed_elementwise_offer::<WIDTH>(operation).and_then(|base| {
                crate::fixed_numeric_value_capacity::value_offer_for_placement(base, placement)
            })
        }
        .map_err(|_| {
            FixedElementwisePreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape)
        })?;
        verify_fixed_placement(placement, &offer)
            .map_err(FixedElementwisePreparationRefusal::Planned)?;
        let ty = fixed_numeric_type(&format!("NumericF32Vector{WIDTH}"))
            .map_err(|_| FixedElementwisePreparationRefusal::Codec(FixedCodecRefusal::Shape))?;
        let codec =
            || FixedF32VectorCodec::prepare(&ty).map_err(FixedElementwisePreparationRefusal::Codec);
        let scalar = if operation.scalars() > 0 {
            Some(
                FixedF32VectorCodec::prepare(&fixed_numeric_type("NumericF32Vector1").map_err(
                    |_| FixedElementwisePreparationRefusal::Codec(FixedCodecRefusal::Shape),
                )?)
                .map_err(FixedElementwisePreparationRefusal::Codec)?,
            )
        } else {
            None
        };
        Ok(Self {
            operation,
            input: codec()?,
            right: if operation.binary() {
                Some(codec()?)
            } else {
                None
            },
            scalar,
            output: codec()?,
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
impl<const WIDTH: usize, const PORTS: usize> StepBack<PORTS> for FixedElementwiseBack<WIDTH> {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 1500,
            });
        }
        self.staged = false;
        if self.committed_frames == u64::MAX {
            return fail(1599);
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        let arity = self.operation.arity();
        if PORTS < arity {
            return fail(1501);
        }
        if (0..arity).any(|p| io.input_closed(PortId(p as u16))) {
            if !self.flow {
                return fail(1501);
            }
            if (0..arity).all(|p| io.input_closed(PortId(p as u16))) {
                return StepOutcome::Complete;
            }
            if (0..arity).any(|p| io.input(PortId(p as u16)).is_some()) {
                return fail(1598);
            }
            return StepOutcome::Await;
        }
        if (0..arity).any(|p| io.input(PortId(p as u16)).is_none()) || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let mut left = [0.; WIDTH];
        let mut right = [0.; WIDTH];
        let mut scalars = [0.; 2];
        let Some(value) = inputs.input(PortId(0)) else {
            return fail(1502);
        };
        if self.input.decode(value, &mut left).is_err() {
            return fail(1503);
        }
        if let Some(codec) = &self.right {
            let Some(value) = inputs.input(PortId(1)) else {
                return fail(1502);
            };
            if codec.decode(value, &mut right).is_err() {
                return fail(1503);
            }
        }
        if let Some(codec) = &self.scalar {
            for (i, result) in scalars
                .iter_mut()
                .enumerate()
                .take(self.operation.scalars())
            {
                let Some(value) = inputs.input(PortId((i + 1) as u16)) else {
                    return fail(1502);
                };
                let mut scalar = [0.];
                if codec.decode(value, &mut scalar).is_err() {
                    return fail(1503);
                }
                *result = scalar[0];
            }
        }
        if self.operation == FixedElementwiseOperation::Clamp && scalars[0] > scalars[1] {
            return fail(1504);
        }
        let mut result = [0.; WIDTH];
        for (i, out) in result.iter_mut().enumerate() {
            *out = match self.operation {
                FixedElementwiseOperation::Add => left[i] + right[i],
                FixedElementwiseOperation::Multiply => left[i] * right[i],
                FixedElementwiseOperation::Sigmoid => {
                    if left[i] >= 0. {
                        1. / (1. + libm::expf(-left[i]))
                    } else {
                        let exponential = libm::expf(left[i]);
                        exponential / (1. + exponential)
                    }
                }
                FixedElementwiseOperation::Complement => 1. - left[i],
                FixedElementwiseOperation::Exp => libm::expf(left[i]),
                FixedElementwiseOperation::Scale => left[i] * scalars[0],
                FixedElementwiseOperation::Clamp => left[i].max(scalars[0]).min(scalars[1]),
                FixedElementwiseOperation::ReciprocalOffset => 1. / (left[i] + scalars[0]),
            };
        }
        if self.output.encode(&result).is_err() {
            return fail(1505);
        }
        for p in 0..arity {
            if io.consume(PortId(p as u16)).is_err() {
                return fail(1506);
            }
        }
        if io
            .send_prepared(PortId(0), self.output.encoded().len() as u32)
            .is_err()
        {
            return fail(1507);
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
