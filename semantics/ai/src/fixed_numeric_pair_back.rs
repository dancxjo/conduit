//! One-invocation Value pairing via the core prepared exact typed-pair encoder.
use crate::fixed_numeric_pair_catalog::fixed_numeric_pair_contracts;
use crate::fixed_numeric_preparation::{FixedPlannedRefusal, verify_fixed_placement};
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, PortId as KPort,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
use conduit_plot::maximum_prepared_canonical_value_bytes;
pub const VALUE_PAIR_IMPLEMENTATION: &str = "conduit.value/scalar-typed-pair@1";
pub fn fixed_value_pair_offer(identity: &str) -> Result<CapabilityOffer, String> {
    let (_, _, kind) = fixed_numeric_pair_contracts()?
        .into_iter()
        .find(|(_, _, kind)| kind.kind_id.as_str() == identity)
        .ok_or_else(|| String::from("unsupported exact Value pair"))?;
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(format!("{VALUE_PAIR_IMPLEMENTATION}/{identity}")),
            execution_profile_id: ExecutionProfileId::from(VALUE_PAIR_IMPLEMENTATION),
            implementation_id: ImplementationId::from(VALUE_PAIR_IMPLEMENTATION),
            artifact_id: ArtifactId::from(VALUE_PAIR_IMPLEMENTATION),
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
pub struct FixedValuePairBack {
    encoder: PreparedTypedTuplePairEncoder,
    left: crate::fixed_numeric_finite_envelope::FiniteEnvelope,
    right: crate::fixed_numeric_finite_envelope::FiniteEnvelope,
    staged: bool,
    finished: bool,
    cancelled: bool,
}
impl FixedValuePairBack {
    pub fn prepare_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
    ) -> Result<Self, FixedPairPreparationRefusal> {
        if PORTS < 2 || fuel < 3 {
            return Err(FixedPairPreparationRefusal::StepBudget);
        }
        let expected = fixed_value_pair_offer(placement.kind_id.as_str())
            .map_err(|_| FixedPairPreparationRefusal::Shape)?;
        verify_fixed_placement(placement, &expected)
            .map_err(FixedPairPreparationRefusal::Planned)?;
        let (_, ty, _) = fixed_numeric_pair_contracts()
            .map_err(|_| FixedPairPreparationRefusal::Shape)?
            .into_iter()
            .find(|(_, _, kind)| kind.kind_id == placement.kind_id)
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
            finished: false,
            cancelled: false,
        })
    }
    pub fn output(&self) -> &[u8] {
        self.encoder.encoded()
    }
    pub fn has_committed_result(&self) -> bool {
        self.finished
    }
}
impl<const PORTS: usize> StepBack<PORTS> for FixedValuePairBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 1300,
            });
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if PORTS < 2 || (0..2).any(|p| io.input_closed(KPort(p))) {
            return fail(1301);
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
            self.finished = true;
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
