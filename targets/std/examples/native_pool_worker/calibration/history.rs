//! One locally validated service observation, never a deserialized estimate.
use super::Prepared;
use conduit_core::*;
use conduit_planner::resource_cost::*;
use conduit_plot::CheckedGear;
use conduit_std_host::{OwnedLocalModelCompletedWork, StdHost, ThreadTimer, TimerAdapter};

pub(crate) struct Retained {
    host: HostAdvertisement,
    receipt: OwnedLocalModelCompletedWork,
    gear: CheckedGear,
    port: PortId,
    input: ValuePayload,
    duration: CostDurationRange,
    window: CostObservationWindow,
    stages: CostForecastStages,
}

impl Retained {
    pub(super) fn capture(
        host: &HostAdvertisement,
        receipt: conduit_std_host::LocalModelCompletedWork<'_>,
        demand: CostDemand<'_>,
        duration: CostDurationRange,
        window: CostObservationWindow,
        stages: CostForecastStages,
    ) -> Result<Self, String> {
        if demand.inputs().len() != 1 || demand.work_estimate().is_some() {
            return Err("retained service demand extent unsupported".into());
        }
        Ok(Self {
            host: host.clone(),
            receipt: receipt.into_owned(),
            gear: demand.class().gear().clone(),
            port: demand.inputs()[0].port.clone(),
            input: demand.inputs()[0].value.clone(),
            duration,
            window,
            stages,
        })
    }

    pub(super) fn prepare(
        &self,
        host: &StdHost,
        demand: CostDemand<'_>,
    ) -> Result<Prepared, String> {
        CostWorkClass::from_checked_gear(&self.gear)
            .validate_demand(demand.class().gear())
            .map_err(|e| format!("retained service class: {e:?}"))?;
        if demand.inputs().len() != 1
            || demand.inputs()[0].port != &self.port
            || demand.inputs()[0].value != &self.input
            || demand.work_estimate().is_some()
        {
            return Err("retained service request changed".into());
        }
        let receipt = self.receipt.as_borrowed();
        let offer = self
            .host
            .capabilities
            .iter()
            .find(|o| o.capability_id == receipt.realization().capability_id)
            .ok_or("retained service offer absent")?;
        let metric = ArtifactId::from("conduit.proof/adapter-service-calibration@1");
        let revision = CostCalibrationRevision::new(&metric, 1)
            .map_err(|e| format!("retained service revision: {e:?}"))?;
        let sign = SignId::from(format!(
            "service-completion/{}",
            receipt.selection().operation_id.as_str()
        ));
        let source = CostEvidenceSource::bind(
            &self.host,
            offer,
            None,
            &sign,
            revision,
            self.window.clone(),
        )
        .map_err(|e| format!("retained service source: {e:?}"))?;
        let owner = host.advertisement();
        let now = ThreadTimer
            .monotonic_observation(&owner.host_id, &owner.boot_id)
            .ok_or("retained service clock unavailable")?;
        source
            .validate_at(owner, revision, &now)
            .map_err(|e| format!("retained service freshness: {e:?}"))?;
        let mut stages = self.stages;
        stages.service = Some(self.duration);
        let forecast =
            FulfillmentCostForecast::sequential(stages, &mut ExactNumericWork::new(100_000_000))
                .map_err(|e| format!("retained service model: {e:?}"))?;
        Ok(Prepared {
            forecast,
            window: self.window.clone(),
            source: Some(CostSourceAttribution::from_source(&source)),
            origin: Some(sign),
        })
    }
}
