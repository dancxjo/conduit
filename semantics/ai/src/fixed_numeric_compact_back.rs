//! Resource-bound unit-domain signed-Q7 tiled linear/affine owners.
//! Quantization and packing are explicit in the selected Fore. No speech policy.
use crate::{
    fixed_compact::{
        CompactMatrixPacking, CompactTensorRefusal, FixedCompactTensorLinear, fixed_signed_q7,
    },
    fixed_numeric_binding::{FixedBindingRefusal, FixedTensorPortBinding},
    fixed_numeric_catalog::fixed_numeric_type,
    fixed_numeric_codec::{FixedCodecRefusal, FixedF32VectorCodec},
    fixed_numeric_compact_catalog::{compact_offer, fixed_compact_type},
    fixed_numeric_preparation::{FixedPlannedRefusal, verify_fixed_placement},
    fixed_tensor_resource::AdmittedFixedTensorResource,
};
use alloc::{format, sync::Arc, vec::Vec};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, PortId,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
#[derive(Debug)]
pub enum CompactPreparationRefusal {
    Shape,
    Budget,
    Planned(FixedPlannedRefusal),
    Codec(FixedCodecRefusal),
    Binding(FixedBindingRefusal),
    Tensor(CompactTensorRefusal),
}
pub struct FixedCompactBack<const INPUT: usize, const OUTPUT: usize> {
    matrix: FixedCompactTensorLinear<'static, INPUT, OUTPUT>,
    input: FixedF32VectorCodec<INPUT>,
    output: FixedF32VectorCodec<OUTPUT>,
    resources: Vec<(PortId, FixedTensorPortBinding)>,
    value_port: PortId,
    flow: bool,
    bound: bool,
    staged: bool,
    cancelled: bool,
    committed: u64,
}
impl<const INPUT: usize, const OUTPUT: usize> FixedCompactBack<INPUT, OUTPUT> {
    pub fn prepare_planned_owned<const PORTS: usize>(
        gear: &PlannedGear,
        fuel: u16,
        flow: bool,
        weights: Arc<AdmittedFixedTensorResource>,
        scales: Arc<AdmittedFixedTensorResource>,
        bias: Option<Arc<AdmittedFixedTensorResource>>,
    ) -> Result<Self, CompactPreparationRefusal> {
        use CompactPreparationRefusal as R;
        let arity = if bias.is_some() { 4 } else { 3 };
        if PORTS < arity || usize::from(fuel) < arity + 1 {
            return Err(R::Budget);
        }
        let offer = compact_offer(INPUT, OUTPUT, bias.is_some(), flow).map_err(|_| R::Shape)?;
        verify_fixed_placement(gear, &offer).map_err(R::Planned)?;
        let ordinal = |name: &str| {
            PortId(
                gear.inputs
                    .iter()
                    .position(|p| p.port_id.as_str() == name)
                    .expect("verified compact port") as u16,
            )
        };
        let input = FixedF32VectorCodec::prepare(
            &fixed_numeric_type(&format!("NumericF32Vector{INPUT}")).map_err(|_| R::Shape)?,
        )
        .map_err(R::Codec)?;
        let output = FixedF32VectorCodec::prepare(
            &fixed_numeric_type(&format!("NumericF32Vector{OUTPUT}")).map_err(|_| R::Shape)?,
        )
        .map_err(R::Codec)?;
        let mut resources = Vec::with_capacity(arity - 1);
        for (name, profile, resource) in [
            (
                "weights",
                fixed_compact_type(&format!("NumericI8TiledMatrixRef{INPUT}x{OUTPUT}"))
                    .map_err(|_| R::Shape)?,
                &weights,
            ),
            (
                "scales",
                fixed_compact_type(&format!("NumericF32ScaleRef{OUTPUT}")).map_err(|_| R::Shape)?,
                &scales,
            ),
        ] {
            resources.push((
                ordinal(name),
                FixedTensorPortBinding::prepare(&profile, resource.tensor()).map_err(R::Binding)?,
            ));
        }
        if let Some(resource) = bias.as_ref() {
            let profile =
                fixed_numeric_type(&format!("NumericF32BiasRef{OUTPUT}")).map_err(|_| R::Shape)?;
            resources.push((
                ordinal("bias"),
                FixedTensorPortBinding::prepare(&profile, resource.tensor()).map_err(R::Binding)?,
            ));
        }
        let matrix = FixedCompactTensorLinear::prepare_owned(
            weights,
            scales,
            bias,
            CompactMatrixPacking::Input4Output8Tiles,
        )
        .map_err(R::Tensor)?;
        Ok(Self {
            matrix,
            input,
            output,
            resources,
            value_port: ordinal("value"),
            flow,
            bound: false,
            staged: false,
            cancelled: false,
            committed: 0,
        })
    }
    pub fn committed_frames(&self) -> u64 {
        self.committed
    }
    pub fn resources_bound(&self) -> bool {
        self.bound
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
impl<const INPUT: usize, const OUTPUT: usize, const PORTS: usize> StepBack<PORTS>
    for FixedCompactBack<INPUT, OUTPUT>
{
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 2700,
            });
        }
        if (!self.flow && self.committed != 0) || (self.flow && io.input_closed(self.value_port)) {
            return StepOutcome::Complete;
        }
        if self.committed == u64::MAX {
            return fail(2701);
        }
        if io.input(self.value_port).is_none() || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if !self.bound {
            for (port, binding) in &self.resources {
                if io.input_closed(*port) {
                    return fail(2702);
                }
                if io.input(*port).is_none() {
                    return StepOutcome::Await;
                }
                if !inputs
                    .input(*port)
                    .is_some_and(|bytes| binding.matches(bytes))
                {
                    return fail(2703);
                }
            }
        }
        let Some(bytes) = inputs.input(self.value_port) else {
            return fail(2704);
        };
        let mut input = [0.; INPUT];
        let mut quantized = [0; INPUT];
        let mut output = [0.; OUTPUT];
        if self.input.decode(bytes, &mut input).is_err()
            || fixed_signed_q7(&input, &mut quantized).is_err()
            || self.matrix.evaluate(&quantized, &mut output).is_err()
            || self.output.encode(&output).is_err()
        {
            return fail(2705);
        }
        if io.consume(self.value_port).is_err() {
            return fail(2706);
        }
        if !self.bound {
            for (port, _) in &self.resources {
                if io.consume(*port).is_err() {
                    return fail(2707);
                }
            }
        }
        if io
            .send_prepared(PortId(0), self.output.encoded().len() as u32)
            .is_err()
        {
            return fail(2708);
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
            self.bound = true;
            self.committed += 1;
        }
    }
    fn cancel(&mut self) {
        self.staged = false;
        self.cancelled = true;
    }
}

impl<const INPUT: usize, const OUTPUT: usize> FixedCompactBack<INPUT, OUTPUT> {
    /// Retained requested local payload capacity. Shared tensor/model Arc owners,
    /// Box root, Arc headers, allocator bookkeeping and stack are separate charges.
    pub fn local_accounted_heap_bytes(&self) -> usize {
        self.input
            .local_accounted_heap_bytes()
            .saturating_add(self.output.local_accounted_heap_bytes())
            .saturating_add(
                self.resources
                    .capacity()
                    .saturating_mul(core::mem::size_of::<(PortId, FixedTensorPortBinding)>()),
            )
            .saturating_add(self.resources.iter().fold(0usize, |total, (_, v)| {
                total.saturating_add(v.local_accounted_heap_bytes())
            }))
    }
}
