//! Ordinary bounded affine Back. It implements one generic operation only.
//! Immutable tensor resources/authority are admitted by the preparation caller;
//! exact queue-visible identities must agree on every invocation.
use crate::fixed_numeric_preparation::{
    admit_tensor_access, verify_affine_placement, FixedPlannedRefusal,
};
use crate::{
    fixed_numeric_binding::{FixedBindingRefusal, FixedTensorPortBinding},
    fixed_numeric_codec::{FixedCodecRefusal, FixedF32VectorCodec},
    fixed_tensor::{FixedMatrixOrder, FixedTensorAffine, FixedTensorRefusal},
};
use conduit_core::{
    AdmittedResourceAccess, PlannedGear, ResourceReferenceBinding, StructuredInfoType,
};
use conduit_data::TensorValue;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedAffineBackPreparationRefusal {
    Tensor(FixedTensorRefusal),
    Codec(FixedCodecRefusal),
    Binding(FixedBindingRefusal),
    Planned(FixedPlannedRefusal),
    StepBudget,
}
/// value/weights/bias input ports 0/1/2; result output port 0.
/// Each result is a single transaction; no progress state advances under pressure.
pub struct FixedAffineBack<'a, const INPUT: usize, const OUTPUT: usize> {
    affine: FixedTensorAffine<'a, INPUT, OUTPUT>,
    input: FixedF32VectorCodec<INPUT>,
    output: FixedF32VectorCodec<OUTPUT>,
    weights: FixedTensorPortBinding,
    bias: FixedTensorPortBinding,
    staged: bool,
    finished: bool,
    cancelled: bool,
    value_port: PortId,
    weight_port: PortId,
    bias_port: PortId,
    access: Option<[AdmittedResourceAccess; 2]>,
}
impl<'a, const INPUT: usize, const OUTPUT: usize> FixedAffineBack<'a, INPUT, OUTPUT> {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        input_type: &StructuredInfoType,
        output_type: &StructuredInfoType,
        weight_profile: &StructuredInfoType,
        bias_profile: &StructuredInfoType,
        weights: &'a TensorValue,
        weight_bytes: &'a [u8],
        bias: &'a TensorValue,
        bias_bytes: &'a [u8],
        order: FixedMatrixOrder,
    ) -> Result<Self, FixedAffineBackPreparationRefusal> {
        let affine = FixedTensorAffine::prepare(weights, weight_bytes, bias, bias_bytes, order)
            .map_err(FixedAffineBackPreparationRefusal::Tensor)?;
        let input = FixedF32VectorCodec::prepare(input_type)
            .map_err(FixedAffineBackPreparationRefusal::Codec)?;
        let output = FixedF32VectorCodec::prepare(output_type)
            .map_err(FixedAffineBackPreparationRefusal::Codec)?;
        let weights = FixedTensorPortBinding::prepare(weight_profile, weights)
            .map_err(FixedAffineBackPreparationRefusal::Binding)?;
        let bias = FixedTensorPortBinding::prepare(bias_profile, bias)
            .map_err(FixedAffineBackPreparationRefusal::Binding)?;
        Ok(Self {
            affine,
            input,
            output,
            weights,
            bias,
            staged: false,
            finished: false,
            cancelled: false,
            value_port: PortId(0),
            weight_port: PortId(1),
            bias_port: PortId(2),
            access: None,
        })
    }
    /// Bound one-shot realization. Caller has adopted exact immutable resource
    /// bytes; this factory verifies the selected Plan identity and access grants.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_planned<const PORTS: usize>(
        placement: &PlannedGear,
        maximum_step_fuel: u16,
        weights: &'a TensorValue,
        weight_bytes: &'a [u8],
        weight_access: &ResourceReferenceBinding,
        bias: &'a TensorValue,
        bias_bytes: &'a [u8],
        bias_access: &ResourceReferenceBinding,
    ) -> Result<Self, FixedAffineBackPreparationRefusal> {
        if PORTS < 3 || maximum_step_fuel < 4 {
            return Err(FixedAffineBackPreparationRefusal::StepBudget);
        }
        verify_affine_placement::<INPUT, OUTPUT>(placement)
            .map_err(FixedAffineBackPreparationRefusal::Planned)?;
        let access = [
            admit_tensor_access(weights, weight_access)
                .map_err(FixedAffineBackPreparationRefusal::Planned)?,
            admit_tensor_access(bias, bias_access)
                .map_err(FixedAffineBackPreparationRefusal::Planned)?,
        ];
        let ty = crate::fixed_numeric_catalog::fixed_numeric_type;
        let input_type = ty(&alloc::format!("NumericF32Vector{INPUT}")).map_err(|_| {
            FixedAffineBackPreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape)
        })?;
        let output_type = ty(&alloc::format!("NumericF32Vector{OUTPUT}")).map_err(|_| {
            FixedAffineBackPreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape)
        })?;
        let weight_profile =
            ty(&alloc::format!("NumericF32MatrixRef{INPUT}x{OUTPUT}")).map_err(|_| {
                FixedAffineBackPreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape)
            })?;
        let bias_profile = ty(&alloc::format!("NumericF32BiasRef{OUTPUT}")).map_err(|_| {
            FixedAffineBackPreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape)
        })?;
        let mut back = Self::prepare(
            &input_type,
            &output_type,
            &weight_profile,
            &bias_profile,
            weights,
            weight_bytes,
            bias,
            bias_bytes,
            FixedMatrixOrder::InputMajor,
        )?;
        let ordinal = |name: &str| {
            PortId(
                placement
                    .inputs
                    .iter()
                    .position(|p| p.port_id.as_str() == name)
                    .expect("verified exact Fore") as u16,
            )
        };
        back.value_port = ordinal("value");
        back.weight_port = ordinal("weights");
        back.bias_port = ordinal("bias");
        back.access = Some(access);
        Ok(back)
    }
    pub fn admitted_access(&self) -> Option<&[AdmittedResourceAccess; 2]> {
        self.access.as_ref()
    }
    pub fn has_committed_result(&self) -> bool {
        self.finished
    }
    pub fn output(&self) -> &[u8] {
        self.output.encoded()
    }
    pub fn allocation_capacity(&self) -> usize {
        self.input.allocation_capacity()
            + self.output.allocation_capacity()
            + self.weights.allocation_capacity()
            + self.bias.allocation_capacity()
    }
}
impl<const INPUT: usize, const OUTPUT: usize, const PORTS: usize> StepBack<PORTS>
    for FixedAffineBack<'_, INPUT, OUTPUT>
{
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 1100,
            });
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if PORTS < 3 {
            return fail(1101);
        }
        // Resource/provider input closure is not a successful numeric result.
        if (0..3).any(|p| io.input_closed(PortId(p))) {
            return fail(1102);
        }
        if (0..3).any(|p| io.input(PortId(p)).is_none()) || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (Some(value), Some(weights), Some(bias)) = (
            inputs.input(self.value_port),
            inputs.input(self.weight_port),
            inputs.input(self.bias_port),
        ) else {
            return fail(1103);
        };
        if !self.weights.matches(weights) || !self.bias.matches(bias) {
            return fail(1104);
        }
        let mut vector = [0.; INPUT];
        let mut result = [0.; OUTPUT];
        if self.input.decode(value, &mut vector).is_err() {
            return fail(1105);
        }
        if self.affine.apply(&vector, &mut result).is_err() {
            return fail(1106);
        }
        if self.output.encode(&result).is_err() {
            return fail(1107);
        }
        for p in 0..3 {
            if io.consume(PortId(p)).is_err() {
                return fail(1108);
            }
        }
        if io
            .send_prepared(PortId(0), self.output.maximum_bytes() as u32)
            .is_err()
        {
            return fail(1109);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.finished = true;
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then(|| self.output.encoded())
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
fn fail(code: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail: code,
    })
}
