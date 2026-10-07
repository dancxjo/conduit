//! Explicit closing-Flow linear Fore; existing Value Backs remain one-shot.
//! Immutable Value tensor bindings are consumed once with the first accepted
//! frame. Subsequent frames reuse admitted custody, never a reset kernel.
use crate::{
    fixed_numeric_binding::FixedTensorPortBinding,
    fixed_numeric_catalog::{fixed_numeric_contracts, fixed_numeric_type},
    fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_linear_back::FixedLinearPreparationRefusal as Refusal,
    fixed_numeric_preparation::{verify_fixed_placement, FixedPlannedRefusal},
    fixed_tensor::FixedMatrixOrder,
    fixed_tensor_linear::FixedTensorLinear,
    fixed_tensor_resource::AdmittedFixedTensorResource,
};
use alloc::{format, string::String, sync::Arc};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub const FLOW_LINEAR_IMPLEMENTATION: &str = "conduit.numeric/flow-linear-f32-scalar@1";
pub fn linear_flow_contract<const INPUT: usize, const OUTPUT: usize>() -> Result<Kind, String> {
    let mut kind = fixed_numeric_contracts()?
        .into_iter()
        .find(|kind| kind.kind_id.as_str() == format!("numeric/linear{INPUT}x{OUTPUT}"))
        .ok_or_else(|| format!("unsupported linear Flow shape {INPUT}x{OUTPUT}"))?;
    kind.kind_id = kind_id(&format!("numeric/flow-linear{INPUT}x{OUTPUT}"));
    kind.kind_contract_revision = KindIdentity::from("conduit.numeric/closing-flow-linear@1");
    kind.inputs
        .iter_mut()
        .find(|p| p.port_id.as_str() == "value")
        .unwrap()
        .temporal = PortTemporal::Flow { closes: true };
    kind.outputs[0].temporal = PortTemporal::Flow { closes: true };
    Ok(kind)
}
pub fn linear_flow_offer<const INPUT: usize, const OUTPUT: usize>(
) -> Result<CapabilityOffer, String> {
    Ok(BackOfferBuilder::new(
        linear_flow_contract::<INPUT, OUTPUT>()?,
        Back {
            capability_id: CapabilityId::from(format!(
                "{FLOW_LINEAR_IMPLEMENTATION}/{INPUT}x{OUTPUT}"
            )),
            execution_profile_id: ExecutionProfileId::from(FLOW_LINEAR_IMPLEMENTATION),
            implementation_id: ImplementationId::from(FLOW_LINEAR_IMPLEMENTATION),
            artifact_id: ArtifactId::from(FLOW_LINEAR_IMPLEMENTATION),
            host_calls: alloc::vec![],
            resource_requirements: alloc::vec![],
            authority_requirements: alloc::vec![],
        },
    )
    .build())
}
pub struct FixedLinearFlowBack<const INPUT: usize, const OUTPUT: usize> {
    linear: FixedTensorLinear<'static, INPUT, OUTPUT>,
    input: FixedF32VectorCodec<INPUT>,
    output: FixedF32VectorCodec<OUTPUT>,
    weights: FixedTensorPortBinding,
    value_port: PortId,
    weight_port: PortId,
    resources_bound: bool,
    staged: bool,
    cancelled: bool,
    committed_frames: u64,
}
impl<const INPUT: usize, const OUTPUT: usize> FixedLinearFlowBack<INPUT, OUTPUT> {
    pub fn prepare_planned_owned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        weights: Arc<AdmittedFixedTensorResource>,
    ) -> Result<Self, Refusal> {
        if PORTS < 2 || fuel < 3 {
            return Err(Refusal::StepBudget);
        }
        let offer = linear_flow_offer::<INPUT, OUTPUT>()
            .map_err(|_| Refusal::Planned(FixedPlannedRefusal::UnsupportedShape))?;
        verify_fixed_placement(placement, &offer).map_err(Refusal::Planned)?;
        let ty = |name: &str| {
            fixed_numeric_type(name)
                .map_err(|_| Refusal::Planned(FixedPlannedRefusal::UnsupportedShape))
        };
        let input = FixedF32VectorCodec::prepare(&ty(&format!("NumericF32Vector{INPUT}"))?)
            .map_err(Refusal::Codec)?;
        let output = FixedF32VectorCodec::prepare(&ty(&format!("NumericF32Vector{OUTPUT}"))?)
            .map_err(Refusal::Codec)?;
        let weight_binding = FixedTensorPortBinding::prepare(
            &ty(&format!("NumericF32MatrixRef{INPUT}x{OUTPUT}"))?,
            weights.tensor(),
        )
        .map_err(Refusal::Binding)?;
        let linear = FixedTensorLinear::prepare_owned(weights, FixedMatrixOrder::InputMajor)
            .map_err(Refusal::Tensor)?;
        let ordinal = |name: &str| {
            PortId(
                placement
                    .inputs
                    .iter()
                    .position(|p| p.port_id.as_str() == name)
                    .unwrap() as u16,
            )
        };
        Ok(Self {
            linear,
            input,
            output,
            weights: weight_binding,
            value_port: ordinal("value"),
            weight_port: ordinal("weights"),
            resources_bound: false,
            staged: false,
            cancelled: false,
            committed_frames: 0,
        })
    }
    pub fn committed_frames(&self) -> u64 {
        self.committed_frames
    }
    pub fn resources_bound(&self) -> bool {
        self.resources_bound
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
impl<const INPUT: usize, const OUTPUT: usize, const PORTS: usize> StepBack<PORTS>
    for FixedLinearFlowBack<INPUT, OUTPUT>
{
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 2600,
            });
        }
        if io.input_closed(self.value_port) {
            return StepOutcome::Complete;
        }
        if self.committed_frames == u64::MAX {
            return fail(2601);
        }
        if io.input(self.value_port).is_none() || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if !self.resources_bound {
            if io.input_closed(self.weight_port) {
                return fail(2602);
            }
            if io.input(self.weight_port).is_none() {
                return StepOutcome::Await;
            }
            let Some(weights) = inputs.input(self.weight_port) else {
                return fail(2603);
            };
            if !self.weights.matches(weights) {
                return fail(2604);
            }
        }
        let Some(value) = inputs.input(self.value_port) else {
            return fail(2605);
        };
        let mut input = [0.; INPUT];
        let mut output = [0.; OUTPUT];
        if self.input.decode(value, &mut input).is_err()
            || self.linear.apply(&input, &mut output).is_err()
            || self.output.encode(&output).is_err()
        {
            return fail(2606);
        }
        if io.consume(self.value_port).is_err() {
            return fail(2607);
        }
        if !self.resources_bound && io.consume(self.weight_port).is_err() {
            return fail(2608);
        }
        if io
            .send_prepared(PortId(0), self.output.encoded().len() as u32)
            .is_err()
        {
            return fail(2609);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then(|| self.output.encoded())
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.resources_bound = true;
            self.committed_frames += 1;
        }
    }
    fn cancel(&mut self) {
        self.staged = false;
        self.cancelled = true;
    }
}

pub fn install_linear_flow_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for mut kind in fixed_numeric_contracts()?
        .into_iter()
        .filter(|kind| kind.kind_id.as_str().starts_with("numeric/linear"))
    {
        kind.kind_id = kind_id(&kind.kind_id.as_str().replacen(
            "numeric/linear",
            "numeric/flow-linear",
            1,
        ));
        kind.kind_contract_revision = KindIdentity::from("conduit.numeric/closing-flow-linear@1");
        for port in kind.inputs.iter_mut().chain(&mut kind.outputs) {
            if port.port_id.as_str() != "weights" {
                port.temporal = PortTemporal::Flow { closes: true };
            }
        }
        startup.insert(conduit_plot::KindSignature {
            kind: String::from(kind.kind_id.as_str()),
            startup_parameters: alloc::vec![],
        })?;
        startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
        profile
            .insert_kind(kind)
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(())
}
