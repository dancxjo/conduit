use crate::timing::{self, Metrics};
use conduit_core::*;
use conduit_planner::resource_cost::*;

pub const WORK_LIMIT: u64 = 100_000_000;
const SOURCE: &str =
    "plot cost_baseline {\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n}\n";
pub struct Fixture {
    pub inventory: &'static str,
    pub hosts: Vec<HostAdvertisement>,
    pub forecast_signs: Vec<SignId>,
    pub current_signs: Vec<SignId>,
    pub assumption_signs: Vec<SignId>,
    gear: conduit_plot::CheckedGear,
    observations: Vec<ResourceObservation>,
    windows: Vec<CostObservationWindow>,
    times: Vec<MonotonicInstant>,
    forecasts: Vec<FulfillmentCostForecast>,
    calibration: ArtifactId,
    pub preparation: Option<Metrics>,
}
impl Fixture {
    pub fn new(count: usize) -> Result<Self, String> {
        Self::prepare(count, "single-production-pulse-offer", |index| {
            HostAdvertisement {
                protocol_version: PROTOCOL_VERSION,
                host_id: format!("cost-baseline/host-{index:02}").into(),
                boot_id: format!("cost-baseline/boot-{index:02}").into(),
                offer_generation: OfferGeneration(1),
                profile: "cost-baseline/controlled-std@1".into(),
                bases: vec![],
                resources: vec![resource_offer(
                    "cost-baseline/timer",
                    TIMER_RESOURCE_CLASS,
                    1,
                )],
                planner_capabilities: vec![],
                capabilities: vec![conduit_std_offers::signal_pulse_offer()],
            }
        })
    }
    #[cfg(feature = "hosted-pipeline")]
    pub fn with_std_inventory(count: usize) -> Result<Self, String> {
        Self::prepare(count, "production-owner-std-reference-inventory", |index| {
            conduit_std_host::reference_advertisement(conduit_std_host::StdHostConfig {
                host_id: format!("cost-baseline/host-{index:02}").into(),
                boot_id: format!("cost-baseline/boot-{index:02}").into(),
                offer_generation: OfferGeneration(1),
            })
        })
    }
    fn prepare(
        count: usize,
        inventory: &'static str,
        publish: impl FnMut(usize) -> HostAdvertisement,
    ) -> Result<Self, String> {
        if ![1, 8, 32].contains(&count) {
            return Err("unsupported fixture cardinality".into());
        }
        let (result, preparation) = timing::measure(|| -> Result<Self, String> {
            let mut plot = conduit_plot::parse_with_startup(
                SOURCE,
                &conduit_signal::signal_startup_catalog(),
                &conduit_signal::signal_profile_catalog(),
            )
            .map_err(|e| format!("checked cost fixture: {e:?}"))?;
            let hosts = (0..count).map(publish).collect::<Vec<_>>();
            let windows = hosts
                .iter()
                .map(|host| {
                    let clock = MonotonicClockIdentity::new(
                        host.host_id.clone(),
                        host.boot_id.clone(),
                        "cost-baseline/clock".into(),
                        TemporalScale::Milliseconds,
                        1,
                        0,
                    )
                    .map_err(|e| format!("clock: {e:?}"))?;
                    CostObservationWindow::admit(
                        MonotonicInstant::new(100, clock.clone()).unwrap(),
                        MonotonicInstant::new(110, clock).unwrap(),
                    )
                    .map_err(|e| format!("window: {e:?}"))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let times = windows
                .iter()
                .map(|window| {
                    MonotonicInstant::new(105, window.observed().clock().clone()).unwrap()
                })
                .collect();
            let observations = hosts
                .iter()
                .enumerate()
                .map(|(index, host)| {
                    let pool = pulse_resource(host)?;
                    Ok(ResourceObservation {
                        host_id: host.host_id.clone(),
                        boot_id: host.boot_id.clone(),
                        offer_generation: host.offer_generation,
                        pool_id: pool.pool_id.clone(),
                        class_id: pool.class_id.clone(),
                        health: ResourceHealth::Ready,
                        unreserved_units: 1,
                        utilized_units: 0,
                        sign_id: SignId::from(format!("cost-baseline/hard-{index:02}")),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let forecasts = (0..count)
                .map(|index| forecast((count - index) as i128))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Self {
                inventory,
                hosts,
                forecast_signs: (0..count)
                    .map(|i| SignId::from(format!("cost-baseline/forecast-{i:02}")))
                    .collect(),
                current_signs: (0..count)
                    .map(|i| SignId::from(format!("cost-baseline/current-{i:02}")))
                    .collect(),
                assumption_signs: (0..count)
                    .map(|i| SignId::from(format!("cost-baseline/assumption-{i:02}")))
                    .collect(),
                gear: plot.gears.remove(0),
                observations,
                windows,
                times,
                forecasts,
                calibration: "cost-baseline/model@1".into(),
                preparation: None,
            })
        });
        let mut fixture = result?;
        fixture.preparation = Some(preparation);
        Ok(fixture)
    }
    pub fn artifact(&self) -> serde_json::Value {
        serde_json::json!({"schema":"conduit.typed-cost-selection-fixture@1", "source":SOURCE,
            "inventory":self.inventory,"offers_per_host":self.hosts.iter().map(|host| host.capabilities.len()).collect::<Vec<_>>(),
            "hosts":self.hosts,"resource_observations":self.observations,
            "forecast_signs":self.forecast_signs,"current_execution_signs":self.current_signs,
            "historical_assumption_signs":self.assumption_signs,"historical_assumption_observed_ticks":100,"historical_assumption_expiry_ticks":104,
            "calibration":self.calibration,"calibration_revision":1,"observed_ticks":100,"expiry_ticks":110,"evaluation_ticks":105,
            "clock_scale":"Milliseconds","clock_resolution_ticks":1,"clock_uncertainty_ticks":0,
            "condition":"Cold","expected_condition":"Cold","arithmetic_work_limit":WORK_LIMIT,
            "forecast_stages_ms":{"setup":1,"queue":2,"service":(1..=self.hosts.len()).rev().collect::<Vec<_>>(),"transfer":3},
            "expected_host":self.hosts.last().unwrap().host_id,"demand_input_count":0,"declared_pulse_count":2,
            "scope":"production pulse Offer/checked Gear and, where labelled, complete production-owner reference std inventories; controlled cost/current-capacity/clock/residency evidence, not physical service/load or multi-machine execution"})
    }
    pub fn prepared<T>(
        &self,
        inspect: impl FnOnce(
            CostDemand<'_>,
            &[CostEvaluationContext<'_>],
            &[TimedResourceObservation<'_>],
            &[CostedForecast<'_>],
        ) -> Result<T, String>,
    ) -> Result<T, String> {
        let calibration = [CostCalibrationRevision::new(&self.calibration, 1)
            .map_err(|e| format!("revision: {e:?}"))?];
        let sources = self
            .hosts
            .iter()
            .enumerate()
            .map(|(i, host)| {
                CostEvidenceSource::bind(
                    host,
                    pulse_offer(host)?,
                    Some(pulse_resource(host)?),
                    &self.forecast_signs[i],
                    calibration[0],
                    self.windows[i].clone(),
                )
                .map_err(|e| format!("forecast source: {e:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let current_sources = self
            .hosts
            .iter()
            .enumerate()
            .map(|(i, host)| {
                CostEvidenceSource::bind(
                    host,
                    pulse_offer(host)?,
                    Some(pulse_resource(host)?),
                    &self.current_signs[i],
                    calibration[0],
                    self.windows[i].clone(),
                )
                .map_err(|e| format!("current source: {e:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let current = current_sources
            .iter()
            .map(|source| {
                CostExecutionObservation::admit(source, CostExecutionCondition::Cold)
                    .map_err(|e| format!("execution observation: {e:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let assumptions = self
            .hosts
            .iter()
            .enumerate()
            .map(|(i, host)| {
                let window = CostObservationWindow::admit(
                    self.windows[i].observed().clone(),
                    MonotonicInstant::new(104, self.windows[i].observed().clock().clone()).unwrap(),
                )
                .unwrap();
                CostEvidenceSource::bind(
                    host,
                    pulse_offer(host)?,
                    Some(pulse_resource(host)?),
                    &self.assumption_signs[i],
                    calibration[0],
                    window,
                )
                .map_err(|e| format!("assumption: {e:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let historical = assumptions
            .iter()
            .map(|source| {
                CostExecutionObservation::admit(source, CostExecutionCondition::Cold)
                    .map_err(|e| format!("historical condition: {e:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let demand = CostDemand::admit(&self.gear, &[]).map_err(|e| format!("demand: {e:?}"))?;
        let forecasts = sources
            .iter()
            .enumerate()
            .map(|(i, source)| {
                CostedForecast::bind(source, demand, &self.forecasts[i])
                    .and_then(|f| f.with_execution_assumption(&historical[i]))
                    .map_err(|e| format!("forecast: {e:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let contexts = self
            .hosts
            .iter()
            .enumerate()
            .map(|(i, host)| CostEvaluationContext {
                host,
                calibrations: &calibration,
                now: &self.times[i],
                execution_observations: &current[i..i + 1],
            })
            .collect::<Vec<_>>();
        let timed = self
            .observations
            .iter()
            .enumerate()
            .map(|(i, observation)| {
                TimedResourceObservation::admit(observation, self.windows[i].clone())
                    .map_err(|e| format!("hard observation: {e:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        inspect(demand, &contexts, &timed, &forecasts)
    }
}
fn pulse_offer(host: &HostAdvertisement) -> Result<&CapabilityOffer, String> {
    let mut matching = host
        .capabilities
        .iter()
        .filter(|offer| offer.kind_id.as_str() == conduit_signal::PULSE_KIND);
    let offer = matching.next().ok_or("missing production pulse Offer")?;
    if matching.next().is_some() {
        return Err("ambiguous pulse inventory".into());
    }
    Ok(offer)
}
fn pulse_resource(host: &HostAdvertisement) -> Result<&ResourceOffer, String> {
    let offer = pulse_offer(host)?;
    let requirement = offer
        .resource_requirements
        .first()
        .ok_or("missing pulse resource")?;
    host.resources
        .iter()
        .find(|pool| pool.class_id == requirement.class_id && pool.capacity_units > 0)
        .ok_or_else(|| "missing compatible pulse pool".into())
}
fn forecast(service: i128) -> Result<FulfillmentCostForecast, String> {
    let mut work = ExactNumericWork::new(WORK_LIMIT);
    let mut point = |value| -> Result<CostDurationRange, String> {
        let duration = CostDuration::admit(
            RationalQuantity::new(
                ExactRational::from_i128(value),
                Unit::resolve("ms").unwrap(),
                QuantityRole::Linear,
            )
            .map_err(|e| format!("quantity: {e:?}"))?,
            &mut work,
        )
        .map_err(|e| format!("duration: {e:?}"))?;
        CostDurationRange::admit(duration, duration, &mut work).map_err(|e| format!("range: {e:?}"))
    };
    let stages = CostForecastStages {
        setup: Some(point(1)?),
        queue: Some(point(2)?),
        service: Some(point(service)?),
        transfer: Some(point(3)?),
    };
    FulfillmentCostForecast::sequential(stages, &mut work)
        .map_err(|e| format!("fulfillment: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_planner::*;
    fn check(
        fixture: &Fixture,
        missing_current: bool,
        budget: u64,
        inspect: impl FnOnce(Result<CostRealizationSelection<'_>, CostSelectionError>),
    ) {
        fixture
            .prepared(|demand, contexts, timed, forecasts| {
                let contexts = contexts
                    .iter()
                    .map(|c| CostEvaluationContext {
                        host: c.host,
                        calibrations: c.calibrations,
                        now: c.now,
                        execution_observations: if missing_current {
                            &[]
                        } else {
                            c.execution_observations
                        },
                    })
                    .collect::<Vec<_>>();
                inspect(select_realization_with_costs(
                    demand,
                    CostSelectionInputs {
                        hosts: &fixture.hosts,
                        contexts: &contexts,
                        advertisements: &[],
                        requirements: &HardRealizationRequirements::default(),
                        observations: timed,
                        tie_policy: &RealizationPolicy::default(),
                        forecasts,
                        intent: PerformanceIntent::Interactive,
                        unavailable: UnavailableCostPolicy::RequireKnown,
                    },
                    &mut ExactNumericWork::new(budget),
                ));
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn current_evidence_and_hard_capacity_are_required_before_favorable_cost() {
        let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
        let mut fixture = Fixture::new(8).unwrap();
        check(&fixture, true, WORK_LIMIT, |result| {
            let Err(CostSelectionError::NoUsableForecast { costs, .. }) = result else {
                panic!("missing current observations admitted")
            };
            assert_eq!(costs.len(), 8);
            assert!(costs
                .iter()
                .all(|c| c.reason == Some(CostEvidenceRefusal::MissingExecutionObservation)));
        });
        fixture.observations.last_mut().unwrap().unreserved_units = 0;
        check(&fixture, false, WORK_LIMIT, |result| {
            let selected = result.unwrap();
            assert_eq!(
                selected.realization.choice.host_id,
                fixture.hosts[6].host_id
            );
            assert_eq!(selected.costs.len(), 7);
            assert_eq!(
                selected
                    .realization
                    .signs
                    .iter()
                    .filter(|s| matches!(
                        s.disposition,
                        RealizationDecisionDisposition::Rejected(_)
                    ))
                    .count(),
                1
            );
        });
        for o in &mut fixture.observations {
            o.unreserved_units = 0;
        }
        check(&fixture, false, 0, |result| {
            let Err(CostSelectionError::HardAdmission(refusal)) = result else {
                panic!("withdrawn fixture admitted")
            };
            assert_eq!(refusal.signs.len(), 8);
        });
    }
    #[test]
    fn expired_same_clock_and_exhausted_arithmetic_do_not_select() {
        let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
        let mut fixture = Fixture::new(8).unwrap();
        check(&fixture, false, 0, |result| {
            let Err(CostSelectionError::Comparison(refusal)) = result else {
                panic!("exhausted comparison must retain its decision account");
            };
            assert_eq!(
                refusal.reason,
                CostEvidenceRefusal::from(ExactNumericRefusal::WorkLimit)
            );
            assert_eq!(refusal.arithmetic_work_limit, 0);
            assert_eq!(refusal.arithmetic_work_spent, 0);
            assert_eq!(refusal.arithmetic_work_remaining, 0);
            assert_eq!(refusal.realization.signs.len(), 8);
        });
        fixture.times[0] = MonotonicInstant::new(110, fixture.times[0].clock().clone()).unwrap();
        check(&fixture, false, WORK_LIMIT, |result| {
            assert!(matches!(
                result,
                Err(CostSelectionError::Evidence(CostEvidenceRefusal::Stale))
            ))
        });
    }
}
