//! Exact measured-work binding to the existing bounded calibration machinery.
//! Preparation only: this neither runs inference nor authenticates a producer.
use super::LocalModelCompletedWork;
use crate::hosted_local_model::LocalModelAdapterTerminal;
use conduit_core::ExactNumericWork;
use conduit_planner::resource_cost::{
    CostCalibrationRecord, CostEvidenceRefusal, CostEvidenceSource, CostExecutionCondition,
    CostExecutionObservation, CostSourceAttribution, CostedForecast,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalModelCompletionCalibrationRefusal {
    UnsuccessfulWork,
    UnsupportedMeasurementScope,
    ExecutionProfileChanged,
    RequestChanged,
    RealizationChanged,
    ObservationTimeChanged,
    ExecutionSampleChanged,
    Evidence(CostEvidenceRefusal),
}
pub struct LocalModelPreparedCalibration<'a> {
    record: CostCalibrationRecord<'a>,
    execution_at_start: Option<CostSourceAttribution>,
}
impl<'a> LocalModelPreparedCalibration<'a> {
    pub fn record(&self) -> &CostCalibrationRecord<'a> {
        &self.record
    }
    pub fn execution_at_start(&self) -> Option<&CostSourceAttribution> {
        self.execution_at_start.as_ref()
    }
}

impl LocalModelCompletedWork<'_> {
    /// The completion source must already cross the producer-trust boundary.
    /// Neither a supplied Sign name nor matching metadata authenticates it.
    /// Clock uncertainty, signed prediction errors, numeric fuel, history and
    /// duplicate-completion limits remain owned by ordinary cost calibration.
    pub fn prepare_calibration<'a>(
        &self,
        forecast: &'a CostedForecast<'a>,
        completion: &'a CostEvidenceSource<'a>,
        execution_at_start: Option<&CostExecutionObservation<'_>>,
        work: &mut ExactNumericWork,
    ) -> Result<LocalModelPreparedCalibration<'a>, LocalModelCompletionCalibrationRefusal> {
        use LocalModelCompletionCalibrationRefusal as R;
        if self.terminal() != LocalModelAdapterTerminal::Produced {
            return Err(R::UnsuccessfulWork);
        }
        // The adapter interval cannot calibrate coordinator queue wait or other
        // work occurring before/after this call. Admit a service-only model.
        let stages = forecast.forecast().stages();
        for stage in [stages.setup, stages.queue, stages.transfer] {
            if !stage.is_some_and(|range| {
                range.lower().seconds() == conduit_core::ExactRational::ZERO
                    && range.upper().seconds() == conduit_core::ExactRational::ZERO
            }) {
                return Err(R::UnsupportedMeasurementScope);
            }
        }
        let realization = self.realization();
        let class = forecast.demand().class().gear();
        if class.kind_id != realization.kind_id
            || class.kind_contract_revision != realization.kind_contract_revision
            || class.inputs != realization.inputs
            || class.outputs != realization.outputs
            || class.configuration != realization.configuration
            || class.semantic_contract != realization.semantic_contract
            || class.terminal_transductions != realization.terminal_transductions
        {
            return Err(R::ExecutionProfileChanged);
        }
        let inputs = forecast.demand().inputs();
        if inputs.len() != 1
            || realization.inputs.len() != 1
            || inputs[0].port != &realization.inputs[0].port_id
            || inputs[0].value.value_kind != realization.inputs[0].value_kind
            || inputs[0].value.encoded.len() != self.input_bytes() as usize
            || conduit_core::semantic_digest(
                "conduit.std/local-model-completed-input@1",
                &inputs[0].value.encoded,
            ) != self.input_digest()
        {
            return Err(R::RequestChanged);
        }
        let host = completion.host();
        let offer = completion.offer();
        if host.host_id != realization.host_id
            || host.boot_id != realization.boot_id
            || host.offer_generation != realization.offer_generation
            || offer.capability_id != realization.capability_id
            || offer.kind_id != realization.kind_id
            || offer.kind_contract_revision != realization.kind_contract_revision
            || offer.implementation.implementation_id != realization.implementation_id
            || offer.implementation.artifact_id != realization.artifact_id
            || offer.implementation.execution_profile_id != realization.execution_profile_id
        {
            return Err(R::RealizationChanged);
        }
        if completion.window().observed() != self.completed() {
            return Err(R::ObservationTimeChanged);
        }
        if completion.resource().is_some_and(|resource| {
            !realization.resources.iter().any(|binding| {
                binding.pool_id == resource.pool_id && binding.class_id == resource.class_id
            })
        }) {
            return Err(R::RealizationChanged);
        }
        forecast
            .demand()
            .class()
            .validate_source(completion)
            .map_err(R::Evidence)?;
        let condition = if let Some(sample) = execution_at_start {
            if sample.source().host() != completion.host()
                || sample.source().offer() != completion.offer()
                || sample.source().resource() != completion.resource()
            {
                return Err(R::ExecutionSampleChanged);
            }
            sample
                .source()
                .validate_at(completion.host(), completion.calibration(), self.started())
                .map_err(R::Evidence)?;
            sample.condition()
        } else {
            CostExecutionCondition::Unknown
        };
        let record =
            CostCalibrationRecord::admit(forecast, completion, self.started(), condition, work)
                .map_err(R::Evidence)?;
        Ok(LocalModelPreparedCalibration {
            record,
            execution_at_start: execution_at_start
                .map(|sample| CostSourceAttribution::from_source(sample.source())),
        })
    }
}

#[cfg(test)]
#[path = "calibration_tests.rs"]
mod tests;
