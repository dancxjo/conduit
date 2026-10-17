//! Read-only projection of the native owner's closed adapter completion.
use super::LocalModelCompletedWork;
use conduit_core::{ExactNumericWork, Plan};
use conduit_observatory::{CostExplanationRefusal, MAXIMUM_COST_EXPLANATION_BYTES};
use conduit_plan_lowering::shared_pool_runtime::CostAwareSharedPoolAdmissionOutcome;
use conduit_planner::resource_cost::PoolForecastPreparationAccount;
use std::fmt::Write;

impl LocalModelCompletedWork<'_> {
    /// Join the actual native completion to its exact dispatch and input.
    /// This reads receipts only; it cannot admit, retry, or execute work.
    /// Adapter latency is not end-to-end fulfillment or queue/warmup Time.
    pub fn render_dispatch_inspection(
        &self,
        plan: &Plan,
        dispatch: &CostAwareSharedPoolAdmissionOutcome,
        forecasts: &[PoolForecastPreparationAccount],
        work: &mut ExactNumericWork,
    ) -> Result<String, CostExplanationRefusal> {
        use CostExplanationRefusal::{Extent, InconsistentSelection};
        let mut text = conduit_observatory::render_pool_dispatch(plan, dispatch, forecasts, work)?;
        if self.selection() != &dispatch.account.evidence {
            return Err(InconsistentSelection);
        }
        let selected = self
            .selection()
            .selected_realization
            .ok_or(InconsistentSelection)?;
        let pool = plan.fragments[0]
            .shared_pools
            .iter()
            .find(|pool| pool.pool_id == self.selection().pool_id)
            .ok_or(InconsistentSelection)?;
        let envelope = pool
            .realization_envelope
            .get(usize::from(selected))
            .ok_or(InconsistentSelection)?;
        let actual = self.realization();
        let clock = self.started().clock();
        if actual.host_id != envelope.host_id
            || actual.boot_id != envelope.boot_id
            || actual.offer_generation != envelope.offer_generation
            || actual.capability_id != envelope.capability_id
            || actual.implementation_id != envelope.implementation_id
            || actual.artifact_id != envelope.artifact_id
            || actual.resources != envelope.resources
            || clock.host_id() != &actual.host_id
            || clock.boot_id() != &actual.boot_id
            || self.completed().clock() != clock
            || self.completed().elapsed_since(self.started()).ok() != Some(self.elapsed())
        {
            return Err(InconsistentSelection);
        }
        let forecast = forecasts
            .iter()
            .find(|f| f.binding.realization == selected)
            .ok_or(CostExplanationRefusal::NoRetainedDecision)?;
        if forecast.configuration_digest
            != conduit_core::pool_operation_configuration_digest(&actual.configuration)
            || forecast.source.execution_profile_id != actual.execution_profile_id
            || forecast.source.kind_id != actual.kind_id
            || forecast.source.kind_contract_revision != actual.kind_contract_revision
            || forecast.inputs.len() != 1
            || actual.inputs.len() != 1
        {
            return Err(InconsistentSelection);
        }
        let input = &forecast.inputs[0];
        if input.port != actual.inputs[0].port_id
            || input.kind != actual.inputs[0].value_kind
            || input.bytes != self.input_bytes() as usize
            || input.digest != self.input_digest()
        {
            return Err(InconsistentSelection);
        }
        // All appended fields are fixed-size numeric/enum values. Reserve a
        // finite suffix before writing, rather than growing then refusing.
        const MAXIMUM_COMPLETION_SUFFIX: usize = 1024;
        if text.len() > MAXIMUM_COST_EXPLANATION_BYTES - MAXIMUM_COMPLETION_SUFFIX {
            return Err(Extent);
        }
        writeln!(text,
            "native_completion_outcome={:?} measured_scope=adapter-call measured_latency_ticks={} scale={:?} started_ticks={} completed_ticks={} resolution_ticks={} uncertainty_ticks={} input_bytes={} output_bytes={} queue_latency=not-measured warmup_latency=not-measured transfer_latency=not-separately-measured",
            self.terminal(), self.elapsed().ticks(), self.elapsed().scale(),
            self.started().ticks(), self.completed().ticks(), clock.resolution_ticks(),
            clock.uncertainty_ticks(), self.input_bytes(), self.output_bytes()
        ).map_err(|_| Extent)?;
        Ok(text)
    }
}
