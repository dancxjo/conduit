//! Explicit closing-Flow embedding Fore; existing Value Backs remain one-shot.
//! Immutable Value tensor bindings are consumed once with the first accepted
//! frame. Subsequent frames reuse admitted custody, never a reset kernel.
use crate::{
    fixed_numeric_binding::FixedTensorPortBinding,
    fixed_numeric_catalog::{fixed_numeric_contracts, fixed_numeric_type},
    fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_operations_back::FixedOperationPreparationRefusal as Refusal,
    fixed_numeric_preparation::{FixedPlannedRefusal, verify_fixed_placement},
    fixed_tensor::FixedTensorEmbedding,
    fixed_tensor_resource::AdmittedFixedTensorResource,
};
use alloc::{format, string::String, sync::Arc};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, PortId,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
pub const FLOW_EMBEDDING_IMPLEMENTATION: &str = "conduit.numeric/flow-embedding-f32-scalar@1";
pub fn embedding_flow_contract<const ROWS: usize, const WIDTH: usize>() -> Result<Kind, String> {
    let mut kind = fixed_numeric_contracts()?
        .into_iter()
        .find(|kind| kind.kind_id.as_str() == format!("numeric/embedding{ROWS}x{WIDTH}"))
        .ok_or_else(|| format!("unsupported embedding Flow shape {ROWS}x{WIDTH}"))?;
    kind.kind_id = kind_id(&format!("numeric/flow-embedding{ROWS}x{WIDTH}"));
    kind.kind_contract_revision = KindIdentity::from("conduit.numeric/closing-flow-embedding@1");
    kind.inputs
        .iter_mut()
        .find(|p| p.port_id.as_str() == "index")
        .unwrap()
        .temporal = PortTemporal::Flow { closes: true };
    kind.outputs[0].temporal = PortTemporal::Flow { closes: true };
    Ok(kind)
}
pub fn embedding_flow_offer<const ROWS: usize, const WIDTH: usize>()
-> Result<CapabilityOffer, String> {
    Ok(BackOfferBuilder::new(
        embedding_flow_contract::<ROWS, WIDTH>()?,
        Back {
            capability_id: CapabilityId::from(format!(
                "{FLOW_EMBEDDING_IMPLEMENTATION}/{ROWS}x{WIDTH}"
            )),
            execution_profile_id: ExecutionProfileId::from(FLOW_EMBEDDING_IMPLEMENTATION),
            implementation_id: ImplementationId::from(FLOW_EMBEDDING_IMPLEMENTATION),
            artifact_id: ArtifactId::from(FLOW_EMBEDDING_IMPLEMENTATION),
            host_calls: alloc::vec![],
            resource_requirements: alloc::vec![],
            authority_requirements: alloc::vec![],
        },
    )
    .build())
}
pub struct FixedEmbeddingFlowBack<const ROWS: usize, const WIDTH: usize> {
    embedding: FixedTensorEmbedding<'static, ROWS, WIDTH>,
    output: FixedF32VectorCodec<WIDTH>,
    weights: FixedTensorPortBinding,
    index_port: PortId,
    weight_port: PortId,
    resources_bound: bool,
    staged: bool,
    cancelled: bool,
    committed_frames: u64,
}
impl<const ROWS: usize, const WIDTH: usize> FixedEmbeddingFlowBack<ROWS, WIDTH> {
    pub fn prepare_planned_owned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
        weights: Arc<AdmittedFixedTensorResource>,
    ) -> Result<Self, Refusal> {
        if PORTS < 2 || fuel < 3 {
            return Err(Refusal::Shape);
        }
        let offer = embedding_flow_offer::<ROWS, WIDTH>()
            .map_err(|_| Refusal::Planned(FixedPlannedRefusal::UnsupportedShape))?;
        verify_fixed_placement(placement, &offer).map_err(Refusal::Planned)?;
        let ty = |name: &str| {
            fixed_numeric_type(name)
                .map_err(|_| Refusal::Planned(FixedPlannedRefusal::UnsupportedShape))
        };
        let output = FixedF32VectorCodec::prepare(&ty(&format!("NumericF32Vector{WIDTH}"))?)
            .map_err(Refusal::Codec)?;
        let weight_binding = FixedTensorPortBinding::prepare(
            &ty(&format!("NumericEmbedding{ROWS}x{WIDTH}"))?,
            weights.tensor(),
        )
        .map_err(Refusal::Binding)?;
        let embedding = FixedTensorEmbedding::prepare_owned(weights).map_err(Refusal::Tensor)?;
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
            embedding,
            output,
            weights: weight_binding,
            index_port: ordinal("index"),
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
impl<const ROWS: usize, const WIDTH: usize, const PORTS: usize> StepBack<PORTS>
    for FixedEmbeddingFlowBack<ROWS, WIDTH>
{
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 2600,
            });
        }
        if io.input_closed(self.index_port) {
            return StepOutcome::Complete;
        }
        if self.committed_frames == u64::MAX {
            return fail(2601);
        }
        if io.input(self.index_port).is_none() || !io.output_ready(PortId(0)) {
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
        let Some(value) = inputs.input(self.index_port) else {
            return fail(2605);
        };
        let Ok(index): Result<[u8; 2], _> = value.try_into() else {
            return fail(2606);
        };
        let mut output = [0.; WIDTH];
        if self
            .embedding
            .lookup(usize::from(u16::from_le_bytes(index)), &mut output)
            .is_err()
            || self.output.encode(&output).is_err()
        {
            return fail(2606);
        }
        if io.consume(self.index_port).is_err() {
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

pub fn install_embedding_flow_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for mut kind in fixed_numeric_contracts()?
        .into_iter()
        .filter(|kind| kind.kind_id.as_str().starts_with("numeric/embedding"))
    {
        kind.kind_id = kind_id(&kind.kind_id.as_str().replacen(
            "numeric/embedding",
            "numeric/flow-embedding",
            1,
        ));
        kind.kind_contract_revision =
            KindIdentity::from("conduit.numeric/closing-flow-embedding@1");
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
