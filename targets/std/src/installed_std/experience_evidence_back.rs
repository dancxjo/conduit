//! Ordinary pure std Back: one admitted request, one inspectable context.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_semantic_catalog::{
    BoundedExperienceEvidenceProjection, EXPERIENCE_EVIDENCE_MAX_BYTES,
    EXPERIENCE_MODEL_REQUEST_MAX_BYTES,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EXPERIENCE_EVIDENCE_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) static MODEL_REQUEST_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EXPERIENCE_MODEL_REQUEST_IMPLEMENTATION,
    budget: model_budget,
    prepare: prepare_model,
};

pub(super) struct ExperienceEvidenceBack {
    model_request: Option<Box<[u8; EXPERIENCE_MODEL_REQUEST_MAX_BYTES]>>,
    model_request_len: usize,
    projection: BoundedExperienceEvidenceProjection,
    staged: bool,
    complete: bool,
}

impl ExperienceEvidenceBack {
    pub(super) fn new() -> Self {
        Self {
            model_request: None,
            model_request_len: 0,
            projection: BoundedExperienceEvidenceProjection::prepare(),
            staged: false,
            complete: false,
        }
    }
}

impl<const PORTS: usize> StepBack<PORTS> for ExperienceEvidenceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if self.model_request.is_some()
            && io.input(PortId(0)).is_none()
            && io.input_closed(PortId(0))
        {
            if io.consume_closed(PortId(0)).is_err() {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 0,
                });
            }
            self.complete = true;
            return StepOutcome::Complete;
        }
        // Do not consume a request or perform work while downstream is pressured.
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(input) = bytes.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        let output = match self.projection.execute(input) {
            Ok(output) => output,
            Err(refusal) => {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: refusal_detail(refusal),
                });
            }
        };
        let length = match self.model_request.as_mut() {
            Some(buffer) => {
                match super::experience_model_request::render(output, buffer.as_mut()) {
                    Ok(length) => {
                        self.model_request_len = length;
                        length
                    }
                    Err(refusal) => {
                        return StepOutcome::Fail(Failure {
                            code: FailureCode::InvalidInput,
                            detail: refusal_detail(refusal),
                        });
                    }
                }
            }
            None => output.len(),
        };
        if !io.output_ready_for(PortId(0), length as u32) {
            return StepOutcome::Await;
        }
        if io.consume(PortId(0)).is_err() || io.send_prepared(PortId(0), length as u32).is_err() {
            return StepOutcome::Fail(Failure {
                code: FailureCode::WorkBudgetExhausted,
                detail: 0,
            });
        }
        self.staged = true;
        StepOutcome::Progress
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == PortId(0)).then(|| match &self.model_request {
            Some(buffer) => &buffer[..self.model_request_len],
            None => self.projection.output(),
        })
    }

    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.complete = self.model_request.is_none();
        }
    }

    fn cancel(&mut self) {
        self.staged = false;
        self.complete = true;
    }
}

// Stable diagnostic identities on the ordinary kernel failure Sign.
pub(super) fn refusal_detail(refusal: conduit_human::ExperienceEvidenceRefusal) -> u16 {
    use conduit_human::ExperienceEvidenceRefusal::*;
    match refusal {
        Malformed => 1,
        ByteBound => 2,
        ItemBound => 3,
        IdentityBound => 4,
        DuplicateSelection => 5,
        DuplicateEvidence => 6,
        UnselectedEvidence => 7,
        MissingEvidence => 8,
        StaleRevision => 9,
        ProvenanceMismatch => 10,
        SourceBound => 11,
        InvalidEpistemicCombination => 12,
        FutureRevision => 13,
        RelationshipBound => 14,
        InvalidRelationship => 15,
    }
}

fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    exact_budget(gear, conduit_std_offers::experience_evidence_offer())
}

fn model_budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    exact_budget(gear, conduit_std_offers::experience_model_request_offer())
}

fn exact_budget(
    gear: &PlannedGear,
    expected: conduit_core::CapabilityOffer,
) -> Result<BackBudget, String> {
    if gear.kind_id != expected.kind_id
        || gear.kind_contract_revision != expected.kind_contract_revision
        || gear.capability_id != expected.capability_id
        || gear.execution_profile_id != expected.implementation.execution_profile_id
        || gear.implementation_id != expected.implementation.implementation_id
        || gear.artifact_id != expected.implementation.artifact_id
        || gear.inputs != expected.inputs
        || gear.outputs != expected.outputs
        || gear.limits != expected.limits
        || gear.semantic_contract != expected.semantic_contract
        || !gear.configuration.is_empty()
        || !gear.host_calls.is_empty()
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
        || gear.base.is_some()
        || !gear.realization_properties.is_empty()
        || !gear.realization_characteristics.is_empty()
        || !gear.pool_references.is_empty()
        || !gear.terminal_transductions.is_empty()
    {
        return Err("evidence projection differs from its exact installed offer".into());
    }
    Ok(BackBudget {
        value_items: 2,
        value_bytes: (EXPERIENCE_EVIDENCE_MAX_BYTES * 2) as u32,
        maximum_value_bytes: EXPERIENCE_EVIDENCE_MAX_BYTES as u32,
        host_requests: 0,
        sign_items: 16,
    })
}

fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    Ok(InstalledBack::ExperienceEvidence(
        ExperienceEvidenceBack::new(),
    ))
}

fn prepare_model(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    model_budget(gear)?;
    let mut back = ExperienceEvidenceBack::new();
    back.model_request = Some(Box::new([0; EXPERIENCE_MODEL_REQUEST_MAX_BYTES]));
    Ok(InstalledBack::ExperienceEvidence(back))
}
