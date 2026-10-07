//! Generic one-invocation numeric Backs. Source owns composition and feedback.
#[cfg(target_has_atomic = "ptr")]
use crate::fixed_tensor_resource::AdmittedFixedTensorResource;
use crate::{
    fixed_neural::{fixed_tanh, FixedNumericRefusal},
    fixed_numeric_binding::{FixedBindingRefusal, FixedTensorPortBinding},
    fixed_numeric_catalog::{fixed_numeric_contracts, fixed_numeric_type},
    fixed_numeric_codec::{FixedCodecRefusal, FixedF32VectorCodec},
    fixed_numeric_preparation::{
        admit_tensor_access, verify_fixed_placement, FixedPlannedRefusal, AFFINE_PROFILE,
    },
    fixed_tensor::{FixedTensorEmbedding, FixedTensorRefusal},
};
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_data::TensorValue;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub const FLOW_OPERATION_IMPLEMENTATION: &str = "conduit.numeric/closing-flow-stateless@1";

#[derive(Debug)]
pub enum FixedOperationPreparationRefusal {
    Planned(FixedPlannedRefusal),
    Codec(FixedCodecRefusal),
    Tensor(FixedTensorRefusal),
    Binding(FixedBindingRefusal),
    Shape,
}

fn offer(kind: &str, implementation: &str) -> Result<CapabilityOffer, String> {
    let selected = fixed_numeric_contracts()?
        .into_iter()
        .find(|item| item.kind_id.as_str() == kind)
        .ok_or_else(|| format!("unsupported numeric operation {kind}"))?;
    Ok(BackOfferBuilder::new(
        selected,
        Back {
            capability_id: CapabilityId::from(format!("{implementation}/{kind}")),
            execution_profile_id: ExecutionProfileId::from(AFFINE_PROFILE),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(implementation),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
pub fn fixed_tanh_offer<const WIDTH: usize>() -> Result<CapabilityOffer, String> {
    offer(
        &format!("numeric/tanh{WIDTH}"),
        "conduit.numeric/scalar-tanh@1",
    )
}
pub fn fixed_concatenate_offer<const LEFT: usize, const RIGHT: usize>(
) -> Result<CapabilityOffer, String> {
    offer(
        &format!("numeric/concatenate{LEFT}x{RIGHT}"),
        "conduit.numeric/concatenate@1",
    )
}
pub fn fixed_embedding_offer<const ROWS: usize, const WIDTH: usize>(
) -> Result<CapabilityOffer, String> {
    offer(
        &format!("numeric/embedding{ROWS}x{WIDTH}"),
        "conduit.numeric/resource-embedding@1",
    )
}
fn admit<const PORTS: usize>(
    placement: &PlannedGear,
    fuel: u16,
    required: usize,
    expected: CapabilityOffer,
) -> Result<(), FixedOperationPreparationRefusal> {
    verify_fixed_placement(placement, &expected)
        .map_err(FixedOperationPreparationRefusal::Planned)?;
    if PORTS < required || usize::from(fuel) < required + 1 {
        return Err(FixedOperationPreparationRefusal::Shape);
    }
    Ok(())
}
fn codec<const WIDTH: usize>(
) -> Result<FixedF32VectorCodec<WIDTH>, FixedOperationPreparationRefusal> {
    let ty = fixed_numeric_type(&format!("NumericF32Vector{WIDTH}"))
        .map_err(|_| FixedOperationPreparationRefusal::Shape)?;
    FixedF32VectorCodec::prepare(&ty).map_err(FixedOperationPreparationRefusal::Codec)
}
#[derive(Default)]
struct Invocation {
    staged: bool,
    finished: bool,
    cancelled: bool,
    flow: bool,
    committed_frames: u64,
}
impl Invocation {
    fn preflight<const PORTS: usize>(
        &mut self,
        io: &StepIo<PORTS>,
        required: usize,
    ) -> Option<StepOutcome> {
        self.staged = false;
        if self.cancelled {
            return Some(if self.flow {
                StepOutcome::Fail(Failure {
                    code: FailureCode::Cancelled,
                    detail: 1200,
                })
            } else {
                fail(1200)
            });
        }
        if self.finished {
            return Some(StepOutcome::Complete);
        }
        if PORTS < required || self.committed_frames == u64::MAX {
            return Some(fail(1201));
        }
        if (0..required).any(|p| io.input_closed(PortId(p as u16))) {
            if !self.flow {
                return Some(fail(1201));
            }
            if (0..required).all(|p| io.input_closed(PortId(p as u16))) {
                return Some(StepOutcome::Complete);
            }
            if (0..required).any(|p| io.input(PortId(p as u16)).is_some()) {
                return Some(fail(1298));
            }
            return Some(StepOutcome::Await);
        }
        if (0..required).any(|p| io.input(PortId(p as u16)).is_none())
            || !io.output_ready(PortId(0))
        {
            return Some(StepOutcome::Await);
        }
        None
    }
    fn committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.finished = !self.flow;
            self.committed_frames += 1;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
fn publish<const PORTS: usize>(
    io: &mut StepIo<PORTS>,
    inputs: usize,
    bytes: usize,
    state: &mut Invocation,
) -> StepOutcome {
    for port in 0..inputs {
        if io.consume(PortId(port as u16)).is_err() {
            return fail(1202);
        }
    }
    if io.send_prepared(PortId(0), bytes as u32).is_err() {
        return fail(1203);
    }
    state.staged = true;
    StepOutcome::Progress
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

pub struct FixedTanhBack<const WIDTH: usize> {
    input: FixedF32VectorCodec<WIDTH>,
    output: FixedF32VectorCodec<WIDTH>,
    state: Invocation,
}
impl<const WIDTH: usize> FixedTanhBack<WIDTH> {
    pub fn prepare_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
    ) -> Result<Self, FixedOperationPreparationRefusal> {
        Self::prepare_internal::<PORTS>(placement, fuel, false)
    }
    pub fn prepare_flow_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
    ) -> Result<Self, FixedOperationPreparationRefusal> {
        Self::prepare_internal::<PORTS>(placement, fuel, true)
    }
    fn prepare_internal<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        flow: bool,
    ) -> Result<Self, FixedOperationPreparationRefusal> {
        let expected = if flow {
            crate::fixed_numeric_temporal::closing_numeric_offer(
                &format!("numeric/tanh{WIDTH}"),
                FLOW_OPERATION_IMPLEMENTATION,
            )
        } else {
            fixed_tanh_offer::<WIDTH>().and_then(|base| {
                crate::fixed_numeric_value_capacity::value_offer_for_placement(base, placement)
            })
        }
        .map_err(|_| FixedOperationPreparationRefusal::Shape)?;
        admit::<PORTS>(placement, fuel, 1, expected)?;
        Ok(Self {
            input: codec()?,
            output: codec()?,
            state: Invocation {
                flow,
                ..Invocation::default()
            },
        })
    }
}
impl<const WIDTH: usize, const PORTS: usize> StepBack<PORTS> for FixedTanhBack<WIDTH> {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(outcome) = self.state.preflight(io, 1) {
            return outcome;
        }
        let Some(bytes) = inputs.input(PortId(0)) else {
            return fail(1204);
        };
        let mut input = [0.0; WIDTH];
        let mut output = [0.0; WIDTH];
        if self.input.decode(bytes, &mut input).is_err()
            || fixed_tanh(&input, &mut output).is_err()
            || self.output.encode(&output).is_err()
        {
            return fail(1205);
        }
        publish(io, 1, self.output.maximum_bytes(), &mut self.state)
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && (!self.state.flow || self.state.staged))
            .then(|| self.output.encoded())
    }
    fn step_committed(&mut self) {
        self.state.committed();
    }
    fn cancel(&mut self) {
        self.state.cancel();
    }
}

pub struct FixedConcatenateBack<const LEFT: usize, const RIGHT: usize, const OUTPUT: usize> {
    left: FixedF32VectorCodec<LEFT>,
    right: FixedF32VectorCodec<RIGHT>,
    output: FixedF32VectorCodec<OUTPUT>,
    state: Invocation,
}
impl<const LEFT: usize, const RIGHT: usize, const OUTPUT: usize>
    FixedConcatenateBack<LEFT, RIGHT, OUTPUT>
{
    pub fn prepare_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
    ) -> Result<Self, FixedOperationPreparationRefusal> {
        Self::prepare_internal::<PORTS>(placement, fuel, false)
    }
    pub fn prepare_flow_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
    ) -> Result<Self, FixedOperationPreparationRefusal> {
        Self::prepare_internal::<PORTS>(placement, fuel, true)
    }
    fn prepare_internal<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        flow: bool,
    ) -> Result<Self, FixedOperationPreparationRefusal> {
        if LEFT.checked_add(RIGHT) != Some(OUTPUT) {
            return Err(FixedOperationPreparationRefusal::Shape);
        }
        let expected = if flow {
            crate::fixed_numeric_temporal::closing_numeric_offer(
                &format!("numeric/concatenate{LEFT}x{RIGHT}"),
                FLOW_OPERATION_IMPLEMENTATION,
            )
        } else {
            fixed_concatenate_offer::<LEFT, RIGHT>()
        }
        .map_err(|_| FixedOperationPreparationRefusal::Shape)?;
        admit::<PORTS>(placement, fuel, 2, expected)?;
        Ok(Self {
            left: codec()?,
            right: codec()?,
            output: codec()?,
            state: Invocation {
                flow,
                ..Invocation::default()
            },
        })
    }
}
impl<const LEFT: usize, const RIGHT: usize, const OUTPUT: usize, const PORTS: usize> StepBack<PORTS>
    for FixedConcatenateBack<LEFT, RIGHT, OUTPUT>
{
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(outcome) = self.state.preflight(io, 2) {
            return outcome;
        }
        let (Some(left), Some(right)) = (inputs.input(PortId(0)), inputs.input(PortId(1))) else {
            return fail(1206);
        };
        let mut a = [0.0; LEFT];
        let mut b = [0.0; RIGHT];
        let mut output = [0.0; OUTPUT];
        if self.left.decode(left, &mut a).is_err() || self.right.decode(right, &mut b).is_err() {
            return fail(1207);
        }
        output[..LEFT].copy_from_slice(&a);
        output[LEFT..].copy_from_slice(&b);
        if self.output.encode(&output).is_err() {
            return fail(1208);
        }
        publish(io, 2, self.output.maximum_bytes(), &mut self.state)
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && (!self.state.flow || self.state.staged))
            .then(|| self.output.encoded())
    }
    fn step_committed(&mut self) {
        self.state.committed();
    }
    fn cancel(&mut self) {
        self.state.cancel();
    }
}

fn decode_index(bytes: &[u8]) -> Result<usize, FixedNumericRefusal> {
    let encoded: [u8; 2] = bytes.try_into().map_err(|_| FixedNumericRefusal::Index)?;
    Ok(usize::from(u16::from_le_bytes(encoded)))
}
pub struct FixedEmbeddingBack<'a, const ROWS: usize, const WIDTH: usize> {
    table: FixedTensorEmbedding<'a, ROWS, WIDTH>,
    access: AdmittedResourceAccess,
    binding: FixedTensorPortBinding,
    output: FixedF32VectorCodec<WIDTH>,
    state: Invocation,
}
impl<'a, const ROWS: usize, const WIDTH: usize> FixedEmbeddingBack<'a, ROWS, WIDTH> {
    #[cfg(target_has_atomic = "ptr")]
    pub fn prepare_planned_owned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        resource: Arc<AdmittedFixedTensorResource>,
    ) -> Result<Self, FixedOperationPreparationRefusal> {
        let expected = fixed_embedding_offer::<ROWS, WIDTH>()
            .map_err(|_| FixedOperationPreparationRefusal::Shape)?;
        admit::<PORTS>(placement, fuel, 2, expected)?;
        let profile = fixed_numeric_type(&format!("NumericEmbedding{ROWS}x{WIDTH}"))
            .map_err(|_| FixedOperationPreparationRefusal::Shape)?;
        let binding = FixedTensorPortBinding::prepare(&profile, resource.tensor())
            .map_err(FixedOperationPreparationRefusal::Binding)?;
        let access = resource.access().clone();
        Ok(Self {
            table: FixedTensorEmbedding::prepare_owned(resource)
                .map_err(FixedOperationPreparationRefusal::Tensor)?,
            access,
            binding,
            output: codec()?,
            state: Invocation::default(),
        })
    }
    pub fn admitted_resource_access(&self) -> &AdmittedResourceAccess {
        &self.access
    }
    pub fn prepare_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        tensor: &'a TensorValue,
        bytes: &'a [u8],
        access: &ResourceReferenceBinding,
    ) -> Result<Self, FixedOperationPreparationRefusal> {
        let expected = fixed_embedding_offer::<ROWS, WIDTH>()
            .map_err(|_| FixedOperationPreparationRefusal::Shape)?;
        admit::<PORTS>(placement, fuel, 2, expected)?;
        let admitted = admit_tensor_access(tensor, access)
            .map_err(FixedOperationPreparationRefusal::Planned)?;
        let table = FixedTensorEmbedding::prepare(tensor, bytes)
            .map_err(FixedOperationPreparationRefusal::Tensor)?;
        let profile = fixed_numeric_type(&format!("NumericEmbedding{ROWS}x{WIDTH}"))
            .map_err(|_| FixedOperationPreparationRefusal::Shape)?;
        let binding = FixedTensorPortBinding::prepare(&profile, tensor)
            .map_err(FixedOperationPreparationRefusal::Binding)?;
        Ok(Self {
            table,
            access: admitted,
            binding,
            output: codec()?,
            state: Invocation::default(),
        })
    }
}
impl<const ROWS: usize, const WIDTH: usize, const PORTS: usize> StepBack<PORTS>
    for FixedEmbeddingBack<'_, ROWS, WIDTH>
{
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(outcome) = self.state.preflight(io, 2) {
            return outcome;
        }
        let (Some(index), Some(weights)) = (inputs.input(PortId(0)), inputs.input(PortId(1)))
        else {
            return fail(1209);
        };
        if !self.binding.matches(weights) {
            return fail(1210);
        }
        let Ok(index) = decode_index(index) else {
            return fail(1211);
        };
        let mut output = [0.0; WIDTH];
        if self.table.lookup(index, &mut output).is_err() || self.output.encode(&output).is_err() {
            return fail(1212);
        }
        publish(io, 2, self.output.maximum_bytes(), &mut self.state)
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && (!self.state.flow || self.state.staged))
            .then(|| self.output.encoded())
    }
    fn step_committed(&mut self) {
        self.state.committed();
    }
    fn cancel(&mut self) {
        self.state.cancel();
    }
}
