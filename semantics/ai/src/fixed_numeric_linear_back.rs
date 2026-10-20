//! One-invocation unbiased projection; source owns every subsequent operation.
#[cfg(target_has_atomic = "ptr")]
use crate::fixed_tensor_resource::AdmittedFixedTensorResource;
use crate::{
    fixed_numeric_binding::{FixedBindingRefusal, FixedTensorPortBinding},
    fixed_numeric_catalog::{fixed_numeric_contract, fixed_numeric_type},
    fixed_numeric_codec::{FixedCodecRefusal, FixedF32VectorCodec},
    fixed_numeric_preparation::{
        admit_tensor_access, verify_fixed_placement, FixedPlannedRefusal, AFFINE_PROFILE,
    },
    fixed_tensor::{FixedMatrixOrder, FixedTensorRefusal},
    fixed_tensor_linear::FixedTensorLinear,
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
pub const LINEAR_IMPLEMENTATION: &str = "conduit.numeric/scalar-resource-linear@1";
pub fn fixed_linear_offer<const INPUT: usize, const OUTPUT: usize>(
) -> Result<CapabilityOffer, String> {
    let identity = format!("numeric/linear{INPUT}x{OUTPUT}");
    let kind = fixed_numeric_contract(&identity)?
        .ok_or_else(|| String::from("unsupported exact linear shape"))?;
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(format!("{LINEAR_IMPLEMENTATION}/{INPUT}x{OUTPUT}")),
            execution_profile_id: ExecutionProfileId::from(AFFINE_PROFILE),
            implementation_id: ImplementationId::from(LINEAR_IMPLEMENTATION),
            artifact_id: ArtifactId::from(LINEAR_IMPLEMENTATION),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedLinearPreparationRefusal {
    Tensor(FixedTensorRefusal),
    Codec(FixedCodecRefusal),
    Binding(FixedBindingRefusal),
    Planned(FixedPlannedRefusal),
    StepBudget,
}
pub struct FixedLinearBack<'a, const INPUT: usize, const OUTPUT: usize> {
    projection: FixedTensorLinear<'a, INPUT, OUTPUT>,
    input: FixedF32VectorCodec<INPUT>,
    output: FixedF32VectorCodec<OUTPUT>,
    binding: FixedTensorPortBinding,
    access: AdmittedResourceAccess,
    staged: bool,
    finished: bool,
    cancelled: bool,
}
impl<'a, const INPUT: usize, const OUTPUT: usize> FixedLinearBack<'a, INPUT, OUTPUT> {
    pub fn prepare_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        tensor: &'a TensorValue,
        bytes: &'a [u8],
        authority: &ResourceReferenceBinding,
    ) -> Result<Self, FixedLinearPreparationRefusal> {
        if PORTS < 2 || fuel < 3 {
            return Err(FixedLinearPreparationRefusal::StepBudget);
        }
        let expected = fixed_linear_offer::<INPUT, OUTPUT>().map_err(|_| {
            FixedLinearPreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape)
        })?;
        verify_fixed_placement(placement, &expected)
            .map_err(FixedLinearPreparationRefusal::Planned)?;
        let ty = |name: String| {
            fixed_numeric_type(&name).map_err(|_| {
                FixedLinearPreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape)
            })
        };
        Ok(Self {
            projection: FixedTensorLinear::prepare(tensor, bytes, FixedMatrixOrder::InputMajor)
                .map_err(FixedLinearPreparationRefusal::Tensor)?,
            input: FixedF32VectorCodec::prepare(&ty(format!("NumericF32Vector{INPUT}"))?)
                .map_err(FixedLinearPreparationRefusal::Codec)?,
            output: FixedF32VectorCodec::prepare(&ty(format!("NumericF32Vector{OUTPUT}"))?)
                .map_err(FixedLinearPreparationRefusal::Codec)?,
            binding: FixedTensorPortBinding::prepare(
                &ty(format!("NumericF32MatrixRef{INPUT}x{OUTPUT}"))?,
                tensor,
            )
            .map_err(FixedLinearPreparationRefusal::Binding)?,
            access: admit_tensor_access(tensor, authority)
                .map_err(FixedLinearPreparationRefusal::Planned)?,
            staged: false,
            finished: false,
            cancelled: false,
        })
    }
    #[cfg(target_has_atomic = "ptr")]
    pub fn prepare_planned_owned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        resource: Arc<AdmittedFixedTensorResource>,
    ) -> Result<Self, FixedLinearPreparationRefusal> {
        if PORTS < 2 || fuel < 3 {
            return Err(FixedLinearPreparationRefusal::StepBudget);
        }
        let expected = fixed_linear_offer::<INPUT, OUTPUT>().map_err(|_| {
            FixedLinearPreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape)
        })?;
        verify_fixed_placement(placement, &expected)
            .map_err(FixedLinearPreparationRefusal::Planned)?;
        let ty = |name: String| {
            fixed_numeric_type(&name).map_err(|_| {
                FixedLinearPreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape)
            })
        };
        let binding = FixedTensorPortBinding::prepare(
            &ty(format!("NumericF32MatrixRef{INPUT}x{OUTPUT}"))?,
            resource.tensor(),
        )
        .map_err(FixedLinearPreparationRefusal::Binding)?;
        let access = resource.access().clone();
        Ok(Self {
            projection: FixedTensorLinear::prepare_owned(resource, FixedMatrixOrder::InputMajor)
                .map_err(FixedLinearPreparationRefusal::Tensor)?,
            input: FixedF32VectorCodec::prepare(&ty(format!("NumericF32Vector{INPUT}"))?)
                .map_err(FixedLinearPreparationRefusal::Codec)?,
            output: FixedF32VectorCodec::prepare(&ty(format!("NumericF32Vector{OUTPUT}"))?)
                .map_err(FixedLinearPreparationRefusal::Codec)?,
            binding,
            access,
            staged: false,
            finished: false,
            cancelled: false,
        })
    }
    pub fn admitted_access(&self) -> &AdmittedResourceAccess {
        &self.access
    }
    pub fn output(&self) -> &[u8] {
        self.output.encoded()
    }
    pub fn has_committed_result(&self) -> bool {
        self.finished
    }
}
impl<const INPUT: usize, const OUTPUT: usize> FixedLinearBack<'_, INPUT, OUTPUT> {
    pub(crate) fn compact_layouts(
        &self,
    ) -> Result<
        (
            conduit_kernel::scheduler::CompactF32Layout,
            conduit_kernel::scheduler::CompactF32Layout,
        ),
        FixedCodecRefusal,
    > {
        let layout = |width: usize, bytes: usize| {
            if width == 0 || width > 1024 || width * 4 > bytes {
                return Err(FixedCodecRefusal::Shape);
            }
            Ok(conduit_kernel::scheduler::CompactF32Layout {
                lanes: width as u16,
                logical_bytes: u32::try_from(bytes).map_err(|_| FixedCodecRefusal::Shape)?,
            })
        };
        Ok((
            layout(INPUT, self.input.maximum_bytes())?,
            layout(OUTPUT, self.output.maximum_bytes())?,
        ))
    }
    pub(crate) fn step_transport<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        inputs: &StepInputBytes<'_, PORTS>,
        mut compact: Option<&mut crate::fixed_numeric_compact::CompactVectorBuffer<OUTPUT>>,
    ) -> StepOutcome {
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 1400,
            });
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if PORTS < 2 || (0..2).any(|p| io.input_closed(PortId(p))) {
            return fail(1401);
        }
        if (0..2).any(|p| io.input(PortId(p)).is_none()) || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(weights) = inputs.input(PortId(1)) else {
            return fail(1402);
        };
        if inputs.input(PortId(0)).is_none()
            && (compact.is_none() || inputs.compact_f32(PortId(0)).is_none())
        {
            return fail(1402);
        }
        let mut input = [0.; INPUT];
        let mut output = [0.; OUTPUT];
        if !self.binding.matches(weights)
            || crate::fixed_numeric_compact::decode(
                &self.input,
                inputs,
                PortId(0),
                &mut input,
                compact.is_some(),
            )
            .is_err()
            || self.projection.apply(&input, &mut output).is_err()
            || crate::fixed_numeric_compact::encode(
                &mut self.output,
                &output,
                io,
                compact.as_deref_mut(),
            )
            .is_err()
        {
            return fail(1403);
        }
        for p in 0..2 {
            if io.consume(PortId(p)).is_err() {
                return fail(1404);
            }
        }
        if crate::fixed_numeric_compact::send(io, self.output.encoded().len(), compact.as_deref())
            .is_err()
        {
            return fail(1405);
        }
        self.staged = true;
        StepOutcome::Progress
    }
}

impl<const INPUT: usize, const OUTPUT: usize, const PORTS: usize> StepBack<PORTS>
    for FixedLinearBack<'_, INPUT, OUTPUT>
{
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.step_transport(io, inputs, None)
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
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

// Reuse retains exact code, codecs and admitted tensor custody. Only a
// successfully drained Value invocation may reset its publication marker.
impl<const INPUT: usize, const OUTPUT: usize, const PORTS: usize>
    conduit_kernel::scheduler::prepared_session::ReusableStepBack<PORTS>
    for FixedLinearBack<'_, INPUT, OUTPUT>
{
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn conduit_kernel::ValueStorage) -> bool {
        self.finished && !self.staged && !self.cancelled
    }
    fn begin_next(&mut self, _: ()) {
        self.finished = false;
    }
}
