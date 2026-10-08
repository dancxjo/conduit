//! Explicit closing-Flow pairing via the core prepared exact typed-pair encoder.
//! Both frame inputs commit together; existing Value pairing remains one-shot.
use crate::fixed_numeric_pair_catalog::fixed_numeric_pair_contracts;
use crate::fixed_numeric_preparation::{FixedPlannedRefusal, verify_fixed_placement};
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, PortId as KPort,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
use conduit_plot::maximum_prepared_canonical_value_bytes;
pub const FLOW_PAIR_IMPLEMENTATION: &str = "conduit.value/closing-flow-typed-pair@1";
pub fn fixed_flow_pair_contract(identity: &str) -> Result<Kind, String> {
    let suffix = identity
        .strip_prefix("numeric/flow-pair")
        .ok_or_else(|| String::from("explicit Flow pair identity required"))?;
    let source_identity = format!("numeric/pair{suffix}");
    let (_, _, mut kind) = fixed_numeric_pair_contracts()?
        .into_iter()
        .find(|(_, _, kind)| kind.kind_id.as_str() == source_identity)
        .ok_or_else(|| String::from("unsupported exact Value pair"))?;
    kind.kind_id = kind_id(identity);
    kind.kind_contract_revision = KindIdentity::from(FLOW_PAIR_IMPLEMENTATION);
    for port in kind.inputs.iter_mut().chain(&mut kind.outputs) {
        port.temporal = PortTemporal::Flow { closes: true };
    }
    Ok(kind)
}
pub fn fixed_flow_pair_offer(identity: &str) -> Result<CapabilityOffer, String> {
    Ok(BackOfferBuilder::new(
        fixed_flow_pair_contract(identity)?,
        Back {
            capability_id: CapabilityId::from(format!("{FLOW_PAIR_IMPLEMENTATION}/{identity}")),
            execution_profile_id: ExecutionProfileId::from(FLOW_PAIR_IMPLEMENTATION),
            implementation_id: ImplementationId::from(FLOW_PAIR_IMPLEMENTATION),
            artifact_id: ArtifactId::from(FLOW_PAIR_IMPLEMENTATION),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedPairPreparationRefusal {
    Planned(FixedPlannedRefusal),
    Shape,
    StepBudget,
}
pub struct FixedFlowPairBack {
    encoder: PreparedTypedTuplePairEncoder,
    left: crate::fixed_numeric_finite_envelope::FiniteEnvelope,
    right: crate::fixed_numeric_finite_envelope::FiniteEnvelope,
    staged: bool,
    committed_frames: u64,
    cancelled: bool,
}
impl FixedFlowPairBack {
    pub fn prepare_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
    ) -> Result<Self, FixedPairPreparationRefusal> {
        if PORTS < 2 || fuel < 3 {
            return Err(FixedPairPreparationRefusal::StepBudget);
        }
        let expected = fixed_flow_pair_offer(placement.kind_id.as_str())
            .map_err(|_| FixedPairPreparationRefusal::Shape)?;
        verify_fixed_placement(placement, &expected)
            .map_err(FixedPairPreparationRefusal::Planned)?;
        let (_, ty, _) = fixed_numeric_pair_contracts()
            .map_err(|_| FixedPairPreparationRefusal::Shape)?
            .into_iter()
            .find(|(_, _, kind)| {
                kind.kind_id.as_str()
                    == placement
                        .kind_id
                        .as_str()
                        .replacen("numeric/flow-pair", "numeric/pair", 1)
            })
            .ok_or(FixedPairPreparationRefusal::Shape)?;
        let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            return Err(FixedPairPreparationRefusal::Shape);
        };
        let left = fields[0].value_type();
        let right = fields[1].value_type();
        let encoder = PreparedTypedTuplePairEncoder::new(
            left.clone(),
            maximum_prepared_canonical_value_bytes(left)
                .map_err(|_| FixedPairPreparationRefusal::Shape)?,
            right.clone(),
            maximum_prepared_canonical_value_bytes(right)
                .map_err(|_| FixedPairPreparationRefusal::Shape)?,
        )
        .map_err(|_| FixedPairPreparationRefusal::Shape)?;
        let left = crate::fixed_numeric_finite_envelope::FiniteEnvelope::prepare(left)
            .map_err(|_| FixedPairPreparationRefusal::Shape)?;
        let right = crate::fixed_numeric_finite_envelope::FiniteEnvelope::prepare(right)
            .map_err(|_| FixedPairPreparationRefusal::Shape)?;
        Ok(Self {
            encoder,
            left,
            right,
            staged: false,
            committed_frames: 0,
            cancelled: false,
        })
    }
    pub fn output(&self) -> &[u8] {
        self.encoder.encoded()
    }
    pub fn committed_frames(&self) -> u64 {
        self.committed_frames
    }
}
impl<const PORTS: usize> StepBack<PORTS> for FixedFlowPairBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 1300,
            });
        }
        if PORTS < 2 || self.committed_frames == u64::MAX {
            return fail(2701);
        }
        let closed = [io.input_closed(KPort(0)), io.input_closed(KPort(1))];
        if closed == [true, true] {
            return StepOutcome::Complete;
        }
        if (closed[0] && io.input(KPort(1)).is_some())
            || (closed[1] && io.input(KPort(0)).is_some())
        {
            return fail(2702);
        }
        if (0..2).any(|p| io.input(KPort(p)).is_none()) || !io.output_ready(KPort(0)) {
            return StepOutcome::Await;
        }
        let (Some(left), Some(right)) = (inputs.input(KPort(0)), inputs.input(KPort(1))) else {
            return fail(1302);
        };
        if self.left.validate(left).is_err() || self.right.validate(right).is_err() {
            return fail(1303);
        }
        if self.encoder.encode(left, right).is_err() {
            return fail(1303);
        }
        for p in 0..2 {
            if io.consume(KPort(p)).is_err() {
                return fail(1304);
            }
        }
        if io
            .send_prepared(KPort(0), self.encoder.encoded().len() as u32)
            .is_err()
        {
            return fail(1305);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.committed_frames += 1;
        }
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        (port == KPort(0)).then(|| self.encoder.encoded())
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

pub fn install_fixed_flow_pair_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (_, _, original) in fixed_numeric_pair_contracts()? {
        let identity = original
            .kind_id
            .as_str()
            .replacen("numeric/pair", "numeric/flow-pair", 1);
        let kind = fixed_flow_pair_contract(&identity)?;
        startup.insert(conduit_plot::KindSignature {
            kind: identity.clone(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(
            &identity,
            CheckedFront::new(vec![], kind.inputs.clone(), kind.outputs.clone(), None),
        )?;
        profile
            .insert_kind(kind)
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(())
}
