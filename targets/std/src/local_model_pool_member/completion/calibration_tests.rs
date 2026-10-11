//! Deterministic scope/clock fixtures, not live inference or authenticated Signs.
use super::*;
use conduit_core::*;
use conduit_planner::resource_cost::*;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn ratio(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn point(us: i128) -> CostDurationRange {
    let duration = CostDuration::admit(
        RationalQuantity::new(
            ExactRational::from_i128(us),
            Unit::resolve("us").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut work(),
    )
    .unwrap();
    CostDurationRange::admit(duration, duration, &mut work()).unwrap()
}
fn prompt(bytes: &[u8]) -> Result<(), CostEvidenceRefusal> {
    validate_primitive_info(TEXT_INFO_ID, bytes).map_err(|_| CostEvidenceRefusal::InvalidWorkInput)
}
struct Fixture {
    host: HostAdvertisement,
    gear: conduit_plot::CheckedGear,
    placement: PlannedGear,
    selection: PoolSelectionEvidence,
    completed_ticks: u64,
    sample_expiry: u64,
}
impl Fixture {
    fn new() -> Self {
        let host = conduit_ai::llm_generate_base_fixtures()[0]
            .advertisement
            .clone();
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profiles = conduit_plot::ProfileCatalog::new();
        conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles).unwrap();
        let gear = conduit_plot::parse_with_startup(
            "plot measured {\n model: llm/generate\n}\n",
            &startup,
            &profiles,
        )
        .unwrap()
        .gears
        .remove(0);
        let offer = &host.capabilities[0];
        // A matched checked-profile fixture. Current dynamic-member preparation
        // still needs to carry this profile before real configured calibration.
        let placement = conduit_core::planned_gear_from_parts! {
            placement_id: "fixture/model".into(), gear_id: gear.gear_id.clone(),
            kind_id: gear.kind_id.clone(), kind_contract_revision: gear.kind_contract_revision.clone(),
            execution_profile_id: offer.implementation.execution_profile_id.clone(),
            configuration: gear.configuration.clone(),
            host_id: host.host_id.clone(), boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation, capability_id: offer.capability_id.clone(),
            implementation_id: offer.implementation.implementation_id.clone(),
            artifact_id: offer.implementation.artifact_id.clone(), base: None,
            realization_characteristics: Vec::new(), limits: offer.limits.clone(),
            inputs: gear.inputs.clone(), outputs: gear.outputs.clone(),
            semantic_contract: gear.semantic_contract.clone(),
            terminal_transductions: gear.terminal_transductions.clone(),
            host_calls: offer.host_calls.clone(), resources: Vec::new(),
            authority: Vec::new(), pool_references: Vec::new(),
        };
        Self {
            host,
            gear,
            placement,
            selection: PoolSelectionEvidence {
                plan_id: "fixture/plan".into(),
                pool_id: "fixture/pool".into(),
                operation_id: "fixture/operation".into(),
                selected_realization: Some(0),
                observation_sign_ids: Vec::new(),
                disposition: PoolSelectionDisposition::Selected,
                sign_id: "fixture/selection".into(),
            },
            completed_ticks: 11_000,
            sample_expiry: 2_000,
        }
    }
    fn run(
        &self,
        input: &[u8],
        queue_us: i128,
        check: impl FnOnce(
            LocalModelCompletedWork<'_>,
            CostedForecast<'_>,
            CostEvidenceSource<'_>,
            CostEvidenceSource<'_>,
        ),
    ) {
        let clock = MonotonicClockIdentity::new(
            self.host.host_id.clone(),
            self.host.boot_id.clone(),
            "fixture/model-clock".into(),
            TemporalScale::Microseconds,
            1,
            3,
        )
        .unwrap();
        let window = |observed, expires| {
            CostObservationWindow::admit(
                MonotonicInstant::new(observed, clock.clone()).unwrap(),
                MonotonicInstant::new(expires, clock.clone()).unwrap(),
            )
            .unwrap()
        };
        let profile = ArtifactId::from("fixture/calibration@1");
        let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
        let signs: [SignId; 3] =
            ["fixture/forecast", "fixture/completion", "fixture/start"].map(Into::into);
        let bind = |index, observed, expires| {
            CostEvidenceSource::bind(
                &self.host,
                &self.host.capabilities[0],
                None,
                &signs[index],
                revision,
                window(observed, expires),
            )
            .unwrap()
        };
        let source = bind(0, 100, 50_000);
        let completion = bind(1, self.completed_ticks, 20_000);
        let sample = bind(2, 900, self.sample_expiry);
        let payload = ValuePayload {
            value_kind: self.gear.inputs[0].value_kind.clone(),
            encoded: input.to_vec(),
        };
        let inputs = [CostInput {
            port: &self.gear.inputs[0].port_id,
            value: &payload,
        }];
        let codec_revision = ArtifactId::from("fixture/utf8-prompt@1");
        let codecs = [
            CostInputCodec::admit(&payload.value_kind, &codec_revision, 65_536, prompt).unwrap(),
        ];
        let demand = CostDemand::admit_with_catalog(&self.gear, &inputs, &[], &codecs).unwrap();
        let forecast = FulfillmentCostForecast::sequential(
            CostForecastStages {
                setup: Some(point(0)),
                queue: Some(point(queue_us)),
                service: Some(point(5_000)),
                transfer: Some(point(0)),
            },
            &mut work(),
        )
        .unwrap();
        let predicted = CostedForecast::bind(&source, demand, &forecast).unwrap();
        let started = MonotonicInstant::new(1_000, clock.clone()).unwrap();
        let completed = MonotonicInstant::new(11_000, clock).unwrap();
        let receipt = LocalModelCompletedWork {
            selection: &self.selection,
            realization: &self.placement,
            elapsed: completed.elapsed_since(&started).unwrap(),
            started,
            completed,
            input_bytes: 7,
            output_bytes: 5,
            input_digest: semantic_digest("conduit.std/local-model-completed-input@1", b"request"),
            terminal: LocalModelAdapterTerminal::Produced,
        };
        check(receipt, predicted, completion, sample);
    }
}
#[test]
fn exact_measured_interval_updates_existing_bounded_calibration() {
    Fixture::new().run(b"request", 0, |receipt, forecast, completion, _| {
        let prepared = receipt
            .prepare_calibration(&forecast, &completion, None, &mut work())
            .unwrap();
        let record = prepared.record();
        assert_eq!(record.condition(), CostExecutionCondition::Unknown);
        assert!(prepared.execution_at_start().is_none());
        assert_eq!(record.elapsed().ticks(), 10_000);
        assert_eq!(record.measured().lower().seconds(), ratio(9992, 1_000_000));
        assert_eq!(record.measured().upper().seconds(), ratio(10008, 1_000_000));
        assert_eq!(record.error_bounds().0.coordinate(), ratio(4992, 1_000_000));
        assert_eq!(record.error_bounds().1.coordinate(), ratio(5008, 1_000_000));
        let mut model = BoundedCostCalibration::admit(CostCalibrationPolicy {
            history_capacity: 2,
            maximum_updates: 2,
            minimum_update_ticks: 1,
        })
        .unwrap();
        model.update(record, &mut work()).unwrap();
        assert_eq!(model.updates(), 1);
        assert_eq!(
            model.update(record, &mut work()),
            Err(CostEvidenceRefusal::DuplicateCompletion)
        );
    });
}
#[test]
fn start_condition_requires_fresh_matching_evidence_and_retains_attribution() {
    Fixture::new().run(b"request", 0, |receipt, forecast, completion, sample| {
        let observed = CostExecutionObservation::admit(
            &sample,
            CostExecutionCondition::Warm {
                residency_revision: 7,
            },
        )
        .unwrap();
        let prepared = receipt
            .prepare_calibration(&forecast, &completion, Some(&observed), &mut work())
            .unwrap();
        assert_eq!(prepared.record().condition(), observed.condition());
        assert_eq!(
            prepared.execution_at_start().unwrap().sign_id,
            *sample.sign()
        );
    });
    let mut fixture = Fixture::new();
    fixture.sample_expiry = 1_000; // right-open expiry at actual start
    fixture.run(b"request", 0, |receipt, forecast, completion, sample| {
        let observed =
            CostExecutionObservation::admit(&sample, CostExecutionCondition::Cold).unwrap();
        assert!(matches!(
            receipt.prepare_calibration(&forecast, &completion, Some(&observed), &mut work()),
            Err(LocalModelCompletionCalibrationRefusal::Evidence(
                CostEvidenceRefusal::Stale
            ))
        ));
    });
}
#[test]
fn changed_request_profile_model_and_completion_time_are_refused() {
    use LocalModelCompletionCalibrationRefusal as R;
    Fixture::new().run(b"changed", 0, |receipt, forecast, completion, _| {
        assert!(matches!(
            receipt.prepare_calibration(&forecast, &completion, None, &mut work()),
            Err(R::RequestChanged)
        ));
    });
    let mut fixture = Fixture::new();
    fixture.placement.configuration.clear();
    fixture.run(b"request", 0, |receipt, forecast, completion, _| {
        assert!(matches!(
            receipt.prepare_calibration(&forecast, &completion, None, &mut work()),
            Err(R::ExecutionProfileChanged)
        ));
    });
    let mut fixture = Fixture::new();
    fixture.placement.artifact_id = "fixture/other-model".into();
    fixture.run(b"request", 0, |receipt, forecast, completion, _| {
        assert!(matches!(
            receipt.prepare_calibration(&forecast, &completion, None, &mut work()),
            Err(R::RealizationChanged)
        ));
    });
    let mut fixture = Fixture::new();
    fixture.completed_ticks += 1;
    fixture.run(b"request", 0, |receipt, forecast, completion, _| {
        assert!(matches!(
            receipt.prepare_calibration(&forecast, &completion, None, &mut work()),
            Err(R::ObservationTimeChanged)
        ));
    });
}
#[test]
fn non_service_cost_failed_work_and_exhausted_numeric_fuel_are_refused() {
    use LocalModelCompletionCalibrationRefusal as R;
    Fixture::new().run(b"request", 1, |receipt, forecast, completion, _| {
        assert!(matches!(
            receipt.prepare_calibration(&forecast, &completion, None, &mut work()),
            Err(R::UnsupportedMeasurementScope)
        ));
    });
    Fixture::new().run(b"request", 0, |mut receipt, forecast, completion, _| {
        for terminal in [
            LocalModelAdapterTerminal::Truncated,
            LocalModelAdapterTerminal::ProviderLost,
            LocalModelAdapterTerminal::Cancelled,
            LocalModelAdapterTerminal::Refused,
            LocalModelAdapterTerminal::InvalidStructuredResult,
        ] {
            receipt.terminal = terminal;
            assert!(matches!(
                receipt.prepare_calibration(&forecast, &completion, None, &mut work()),
                Err(R::UnsuccessfulWork)
            ));
        }
        receipt.terminal = LocalModelAdapterTerminal::Produced;
        assert!(matches!(
            receipt.prepare_calibration(
                &forecast,
                &completion,
                None,
                &mut ExactNumericWork::new(0)
            ),
            Err(R::Evidence(CostEvidenceRefusal::Numeric(
                ExactNumericRefusal::WorkLimit
            )))
        ));
    });
}

#[test]
fn same_kind_does_not_allow_another_boot_generation_or_start_offer() {
    use LocalModelCompletionCalibrationRefusal as R;
    for change_boot in [true, false] {
        let mut fixture = Fixture::new();
        if change_boot {
            fixture.placement.boot_id = "fixture/another-boot".into();
        } else {
            fixture.placement.offer_generation.0 += 1;
        }
        fixture.run(b"request", 0, |receipt, forecast, completion, _| {
            assert!(matches!(
                receipt.prepare_calibration(&forecast, &completion, None, &mut work()),
                Err(R::RealizationChanged)
            ));
        });
    }
    Fixture::new().run(b"request", 0, |receipt, forecast, completion, sample| {
        let mut changed = sample.host().clone();
        changed.capabilities[0].implementation.artifact_id = "fixture/another-model".into();
        let foreign = CostEvidenceSource::bind(
            &changed,
            &changed.capabilities[0],
            None,
            sample.sign(),
            sample.calibration(),
            sample.window().clone(),
        )
        .unwrap();
        let observed = CostExecutionObservation::admit(
            &foreign,
            CostExecutionCondition::Warm {
                residency_revision: 7,
            },
        )
        .unwrap();
        assert!(matches!(
            receipt.prepare_calibration(&forecast, &completion, Some(&observed), &mut work()),
            Err(R::ExecutionSampleChanged)
        ));
    });
}
