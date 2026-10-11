//! Exercise the real generic selector, then inspect its historical projection.
use crate::*;
use alloc::{format, vec, vec::Vec};
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use conduit_plot::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};

fn gear() -> conduit_plot::CheckedGear {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "fixture/report-work".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: "fixture/report-work".into(),
            kind_contract_revision: "fixture/report-work@1".into(),
            inputs: vec![PortDescriptor {
                port_id: "request".into(),
                value_kind: TEXT_INFO_ID.into(),
                direction: PortDirection::Input,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            }],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    conduit_plot::parse_with_startup(
        "plot report {\n job: fixture/report-work\n}\n",
        &startup,
        &profile,
    )
    .unwrap()
    .gears
    .remove(0)
}
fn window(host: &HostAdvertisement, expires: u64) -> CostObservationWindow {
    let clock = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "cost-clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    CostObservationWindow::admit(
        MonotonicInstant::new(100, clock.clone()).unwrap(),
        MonotonicInstant::new(expires, clock).unwrap(),
    )
    .unwrap()
}
#[derive(Clone, Copy)]
enum CalibrationMode {
    None,
    Fresh,
    Expired,
}
fn exercise(
    require_known: bool,
    calibration_mode: CalibrationMode,
    inspect: impl FnOnce(Result<CostRealizationSelection<'_>, CostSelectionError>),
) {
    let calibrated = !matches!(calibration_mode, CalibrationMode::None);
    let expired_calibration = matches!(calibration_mode, CalibrationMode::Expired);
    let gear = gear();
    let value = ValuePayload {
        value_kind: TEXT_INFO_ID.into(),
        encoded: b"REQUEST_CANARY_do_not_print".to_vec(),
    };
    let inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &value,
    }];
    let demand = CostDemand::admit(&gear, &inputs).unwrap();
    let mut hosts = (0..4)
        .map(|index| {
            let mut host = conduit_signal_conformance::pico_local_advertisement();
            let mut offer = host
                .capabilities
                .iter()
                .find(|offer| offer.kind_id.as_str() == conduit_signal::PULSE_KIND)
                .unwrap()
                .clone();
            offer.kind_id = gear.kind_id.clone();
            offer.kind_contract_revision = gear.kind_contract_revision.clone();
            offer.startup_parameters = gear.startup_parameters.clone();
            offer.inputs = gear.inputs.clone();
            offer.outputs = gear.outputs.clone();
            offer.semantic_contract = gear.semantic_contract.clone();
            offer.capability_id = format!("fixture/report-{index}").into();
            offer.limits.max_queue_bytes = 4096;
            host.host_id = format!("fixture/report-host-{index}").into();
            host.capabilities = vec![offer];
            host
        })
        .collect::<Vec<_>>();
    hosts[0].capabilities[0].limits.max_queue_bytes = value.encoded.len() as u32 - 1;
    let signs: [SignId; 3] =
        ["forecast/hard-rejected", "forecast/stale", "forecast/fresh"].map(Into::into);
    let calibration = ArtifactId::from("fixture/report-calibration@1");
    let revisions = [CostCalibrationRevision::new(&calibration, 1).unwrap()];
    let sources = hosts
        .iter()
        .take(3)
        .zip(&signs)
        .enumerate()
        .map(|(index, (host, sign))| {
            CostEvidenceSource::bind(
                host,
                &host.capabilities[0],
                None,
                sign,
                revisions[0],
                window(
                    host,
                    if index == 1 || require_known {
                        149
                    } else {
                        200
                    },
                ),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut work = ExactNumericWork::new(100_000_000);
    let third = ExactRational::ratio(1, 3, &mut work).unwrap();
    let duration = |coordinate, work: &mut ExactNumericWork| {
        CostDuration::admit(
            RationalQuantity::new(
                coordinate,
                Unit::resolve("ms").unwrap(),
                QuantityRole::Linear,
            )
            .unwrap(),
            work,
        )
        .unwrap()
    };
    let zero = duration(ExactRational::ZERO, &mut work);
    let service = duration(third, &mut work);
    let zero = CostDurationRange::admit(zero, zero, &mut work).unwrap();
    let service = CostDurationRange::admit(service, service, &mut work).unwrap();
    let forecast = FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: Some(zero),
            queue: Some(zero),
            service: Some(service),
            transfer: Some(zero),
        },
        &mut work,
    )
    .unwrap();
    let initial_samples = sources
        .iter()
        .map(|source| CostedForecast::bind(source, demand, &forecast).unwrap())
        .collect::<Vec<_>>();
    let observations = hosts
        .iter()
        .flat_map(|host| {
            host.resources
                .iter()
                .enumerate()
                .map(move |(index, pool)| ResourceObservation {
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    offer_generation: host.offer_generation,
                    pool_id: pool.pool_id.clone(),
                    class_id: pool.class_id.clone(),
                    health: ResourceHealth::Ready,
                    unreserved_units: pool.capacity_units,
                    utilized_units: 0,
                    sign_id: format!("hard/{}/{index}", host.host_id.as_str()).into(),
                })
        })
        .collect::<Vec<_>>();
    let timed = observations
        .iter()
        .map(|observation| {
            let host = hosts
                .iter()
                .find(|host| host.host_id == observation.host_id)
                .unwrap();
            TimedResourceObservation::admit(observation, window(host, 200)).unwrap()
        })
        .collect::<Vec<_>>();
    let now = hosts
        .iter()
        .map(|host| {
            MonotonicInstant::new(
                if calibrated { 170 } else { 150 },
                window(host, 200).observed().clock().clone(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let completion_sign = SignId::from("completion/measured");
    let clock = sources[2].window().observed().clock().clone();
    let completed = CostEvidenceSource::bind(
        &hosts[2],
        &hosts[2].capabilities[0],
        None,
        &completion_sign,
        revisions[0],
        CostObservationWindow::admit(
            MonotonicInstant::new(160, clock.clone()).unwrap(),
            MonotonicInstant::new(if expired_calibration { 169 } else { 200 }, clock.clone())
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let dispatched = MonotonicInstant::new(110, clock).unwrap();
    let measurement = CostCalibrationRecord::admit(
        &initial_samples[2],
        &completed,
        &dispatched,
        CostExecutionCondition::Cold,
        &mut work,
    )
    .unwrap();
    let mut model = BoundedCostCalibration::admit(CostCalibrationPolicy {
        history_capacity: 2,
        maximum_updates: 2,
        minimum_update_ticks: 10,
    })
    .unwrap();
    model.update(&measurement, &mut work).unwrap();
    let calibration_admission = if expired_calibration {
        completed.window().observed()
    } else {
        &now[2]
    };
    let estimate = if calibrated {
        model
            .snapshot_at(&hosts[2], revisions[0], calibration_admission)
            .unwrap()
    } else {
        None
    };
    let samples = sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let sample = CostedForecast::bind(source, demand, &forecast).unwrap();
            if index == 2 {
                if let Some(estimate) = &estimate {
                    return sample
                        .with_calibration(
                            estimate,
                            CostExecutionCondition::Cold,
                            calibration_admission,
                        )
                        .unwrap();
                }
            }
            sample
        })
        .collect::<Vec<_>>();
    let requirements = HardRealizationRequirements::default();
    let policy = RealizationPolicy::default();
    let current_sign = SignId::from("condition/current");
    let current_source = CostEvidenceSource::bind(
        &hosts[2],
        &hosts[2].capabilities[0],
        None,
        &current_sign,
        revisions[0],
        window(&hosts[2], 200),
    )
    .unwrap();
    let current_execution =
        [CostExecutionObservation::admit(&current_source, CostExecutionCondition::Cold).unwrap()];
    let contexts = hosts
        .iter()
        .zip(&now)
        .map(|(host, now)| CostEvaluationContext {
            host,
            calibrations: &revisions,
            execution_observations: if calibrated && host.host_id == hosts[2].host_id {
                &current_execution
            } else {
                &[]
            },
            now,
        })
        .collect::<Vec<_>>();
    inspect(select_realization_with_costs(
        demand,
        CostSelectionInputs {
            hosts: &hosts,
            contexts: &contexts,
            advertisements: &[],
            requirements: &requirements,
            observations: &timed,
            tie_policy: &policy,
            forecasts: &samples,
            intent: PerformanceIntent::Interactive,
            unavailable: if require_known {
                UnavailableCostPolicy::RequireKnown
            } else {
                UnavailableCostPolicy::PreferKnown
            },
        },
        &mut work,
    ));
}

#[test]
fn actual_selection_projection_keeps_refusals_sources_exact_units_and_redaction() {
    exercise(false, CalibrationMode::None, |result| {
        let result = result.unwrap();
        assert_eq!(
            result.realization.choice.host_id.as_str(),
            "fixture/report-host-2"
        );
        let output = render_cost_selection_explanation(&result).unwrap();
        for required in [
            "historical planner cost decision",
            "not deadlines",
            "QueueByteBound",
            "cost=unusable reason=Stale",
            "sign=forecast/stale",
            "sign=forecast/fresh",
            "cost=unknown",
            "stage service=1/3 ms",
            "effective_fulfillment_seconds=[1/3000",
            "observed=",
            "expires=",
            "evaluated_at=",
            "arithmetic_limb_work=",
        ] {
            assert!(output.contains(required), "missing {required}: {output}");
        }
        assert!(!output.contains("REQUEST_CANARY"));
        assert!(!format!("{result:?}").contains("REQUEST_CANARY"));
        assert!(output.len() <= MAXIMUM_COST_EXPLANATION_BYTES);
    });
}

#[test]
fn refused_cost_selection_retains_stale_sign_and_evaluation_custody() {
    exercise(true, CalibrationMode::None, |result| {
        let error = result.unwrap_err();
        let rendered = render_cost_selection_refusal(&error).unwrap();
        for required in [
            "no cost-admitted winner",
            "QueueByteBound",
            "forecast/stale",
            "cost=unknown",
            "window:",
            "offer_generation:",
        ] {
            assert!(
                rendered.contains(required),
                "missing {required}: {rendered}"
            );
        }
        assert!(!rendered.contains("REQUEST_CANARY"));
        assert!(!format!("{error:?}").contains("REQUEST_CANARY"));
        let CostSelectionError::NoUsableForecast { realization, costs } = error else {
            panic!("expected refusal")
        };
        assert!(matches!(
            realization.signs[0].disposition,
            RealizationDecisionDisposition::Rejected(RealizationRejection::QueueByteBound)
        ));
        assert_eq!(costs[0].reason, Some(CostEvidenceRefusal::Stale));
        assert_eq!(
            costs[0].submitted.as_ref().unwrap().sign_id.as_str(),
            "forecast/stale"
        );
        assert_eq!(costs[0].evaluated_at.ticks(), 150);
        assert!(costs[2].submitted.is_none());
        assert!(costs[2].reason.is_none());
    });
}

#[test]
fn inconsistent_or_overextent_public_decisions_are_refused() {
    assert_eq!(
        render_cost_selection_refusal(&CostSelectionError::Evidence(CostEvidenceRefusal::Stale)),
        Err(CostExplanationRefusal::NoRetainedDecision)
    );
    exercise(false, CalibrationMode::None, |result| {
        let mut result = result.unwrap();
        result.costs[0].selected = true;
        assert_eq!(
            render_cost_selection_explanation(&result),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
        result.costs[0].selected = false;
        result.realization.signs[0].disposition = RealizationDecisionDisposition::Selected;
        assert_eq!(
            render_cost_selection_explanation(&result),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
        while result.realization.signs.len() <= MAXIMUM_COST_CANDIDATES {
            result
                .realization
                .signs
                .push(result.realization.signs[0].clone());
        }
        assert_eq!(
            render_cost_selection_explanation(&result),
            Err(CostExplanationRefusal::Extent)
        );
    });
}

#[test]
fn calibrated_projection_keeps_original_stages_measurement_and_signed_errors() {
    exercise(false, CalibrationMode::Fresh, |result| {
        let output = render_cost_selection_explanation(&result.unwrap()).unwrap();
        for required in [
            "sign=forecast/fresh",
            "sign=completion/measured",
            "updates=1 condition=Cold",
            "current_execution sign=condition/current",
            "current_execution_condition=Cold",
            "stage service=1/3 ms",
            "stage measured=",
            "measured_elapsed=",
            "prediction_error_seconds=[",
            "signed measured minus predicted",
            "effective_fulfillment_seconds=[6/125,13/250]",
        ] {
            assert!(output.contains(required), "missing {required}: {output}");
        }
        assert!(!output.contains("REQUEST_CANARY"));
    });
}

#[test]
fn unusable_calibration_preserves_its_distinct_completion_sign_and_window() {
    exercise(false, CalibrationMode::Expired, |result| {
        let result = result.unwrap();
        assert!(matches!(
            result.costs[1].availability,
            CostAvailability::Unusable(CostEvidenceRefusal::Stale)
        ));
        assert_eq!(
            result.costs[1].submitted_source.unwrap().sign().as_str(),
            "forecast/fresh"
        );
        assert_eq!(
            result.costs[1]
                .submitted_calibration_source
                .unwrap()
                .sign()
                .as_str(),
            "completion/measured"
        );
        let output = render_cost_selection_explanation(&result).unwrap();
        assert!(output.contains("submitted_calibration sign=completion/measured"));
        assert!(output.contains("cost=unusable reason=Stale"));
        assert!(!output.contains("REQUEST_CANARY"));
    });
}

#[test]
fn known_calibration_without_its_current_state_cannot_be_presented_as_valid() {
    exercise(false, CalibrationMode::Fresh, |result| {
        let mut result = result.unwrap();
        let calibrated = result.costs.iter_mut().find(|cost| matches!(
            &cost.availability, CostAvailability::Known(sample) if sample.calibration().is_some()
        )).unwrap();
        calibrated.current_execution = None;
        assert_eq!(
            render_cost_selection_explanation(&result),
            Err(CostExplanationRefusal::InconsistentSelection)
        );
    });
}
