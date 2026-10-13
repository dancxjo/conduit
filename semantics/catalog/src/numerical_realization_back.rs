//! The selected numeric certificate is rechecked and its result frozen before Play.
use alloc::{format, string::String, vec::Vec};
use conduit_core::{
    ConfigurationValue, ExactNumericWork, NumericalAlgorithm, NumericalRealizationRequest,
    PlannedGear, PreparedNumericalRealization,
};
use conduit_kernel::{
    scheduler::{SchedulerError, StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub struct NumericalRealizationBack {
    output: Vec<u8>,
    staged: bool,
    complete: bool,
}
impl NumericalRealizationBack {
    pub fn prepare(gear: &PlannedGear, algorithm: NumericalAlgorithm) -> Result<Self, String> {
        let kind = super::numerical_realize_semantic_contract();
        if gear.kind_id.as_str() != super::NUMERICAL_REALIZE_KIND
            || gear.kind_contract_revision.as_str() != super::NUMERICAL_REALIZE_REVISION
            || gear.inputs != kind.inputs
            || gear.outputs != kind.outputs
            || gear.semantic_contract != kind.semantic_contract()
            || !gear.host_calls.is_empty()
            || !gear.resources.is_empty()
            || !gear.authority.is_empty()
        {
            return Err("numeric Back requires its exact Kind".into());
        }
        if gear.configuration.len() != 1 {
            return Err("numeric Back requires one exact request".into());
        }
        let mut work = ExactNumericWork::new(100_000_000);
        let request = gear
            .configuration
            .iter()
            .find(|entry| entry.key == "request")
            .and_then(|entry| match &entry.value {
                ConfigurationValue::Structured(v) => Some(v),
                _ => None,
            })
            .ok_or("numeric request missing")?;
        if request.profile()
            != super::numerical_request_type()
                .profile()
                .unwrap()
                .value_kind()
        {
            return Err("numeric request Type differs from the Kind Fore".into());
        }
        if request.canonical_value().len() > super::NUMERICAL_REQUEST_MAX_BYTES as usize {
            return Err("numeric request extent".into());
        }
        if !conduit_core::valid_realization_properties(&gear.realization_properties) {
            return Err("invalid numeric realization properties".into());
        }
        let request = NumericalRealizationRequest::from_configuration(request, &mut work)
            .map_err(|why| format!("numeric request: {why:?}"))?;
        let property = gear
            .realization_properties
            .iter()
            .find(|p| *p.profile() == conduit_core::numerical_realization_profile())
            .ok_or("numeric certificate missing")?;
        let prepared = PreparedNumericalRealization::from_property(property, &mut work)
            .map_err(|why| format!("numeric certificate: {why:?}"))?;
        if prepared.algorithm() != algorithm {
            return Err("selected numeric algorithm differs from initialized Back".into());
        }
        prepared
            .admits(&request, &mut work)
            .map_err(|why| format!("numeric requirements: {why:?}"))?;
        let output = super::numerical_result_value(&prepared)?
            .canonical_bytes()
            .map_err(|why| format!("numeric result: {why:?}"))?;
        if output.len() > super::NUMERICAL_RESULT_MAX_BYTES as usize
            || output.len() > gear.limits.max_queue_bytes as usize
            || gear.limits.max_queue_items == 0
            || gear.limits.max_active_instances == 0
        {
            return Err("numeric result extent".into());
        }
        Ok(Self {
            output,
            staged: false,
            complete: false,
        })
    }
    pub fn result_bytes(&self) -> &[u8] {
        &self.output
    }
}
impl<const PORTS: usize> StepBack<PORTS> for NumericalRealizationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if let Err(why) = io.send_prepared(PortId(0), self.output.len() as u32) {
            return StepOutcome::Fail(Failure {
                code: if why == SchedulerError::StepFuelExceeded {
                    FailureCode::WorkBudgetExhausted
                } else {
                    FailureCode::StorageExhausted
                },
                detail: 1,
            });
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then_some(self.output.as_slice())
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.complete = true;
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.complete = true;
        self.staged = false;
    }
}
