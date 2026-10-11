//! Portable structured demand and complete-input hard admission.
mod common;
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
use conduit_plot::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};

fn request_type(identity: &str) -> StructuredInfoType {
    StructuredInfoType::record(
        identity.into(),
        vec![
            StructuredFieldType::new(
                "content",
                StructuredInfoType::leaf(TEXT_INFO_ID.into()).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new(
                "work",
                StructuredInfoType::leaf(COUNT_INFO_ID.into()).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
fn payload(identity: &str, content: &str, work: u64) -> ValuePayload {
    let value = StructuredInfoValue::record(
        request_type(identity),
        vec![
            StructuredFieldValue::new(
                "content",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(TEXT_INFO_ID.into()).unwrap(),
                    content.as_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "work",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(COUNT_INFO_ID.into()).unwrap(),
                    work.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    ValuePayload {
        value_kind: identity.into(),
        encoded: value.canonical_bytes().unwrap(),
    }
}
fn gear(identity: &str) -> conduit_plot::CheckedGear {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "fixture/input-work".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: "fixture/input-work".into(),
            kind_contract_revision: "fixture/input-work@1".into(),
            inputs: vec![PortDescriptor {
                port_id: "request".into(),
                value_kind: identity.into(),
                direction: PortDirection::Input,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            }],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    conduit_plot::parse_with_startup(
        "plot work {\n job: fixture/input-work\n}\n",
        &startup,
        &profile,
    )
    .unwrap()
    .gears
    .remove(0)
}
#[test]
fn checked_structured_schema_admits_exact_demand_without_primitive_reinterpretation() {
    let identity = "fixture/model-request@1";
    let gear = gear(identity);
    let schema = [CostStructuredInputSchema::admit(&request_type(identity), 4096).unwrap()];
    let value = payload(identity, "a finite model-shaped request", 12);
    let inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &value,
    }];
    assert!(matches!(
        CostDemand::admit(&gear, &inputs),
        Err(CostEvidenceRefusal::InvalidWorkInput)
    ));
    let demand = CostDemand::admit_with_structured(&gear, &inputs, &schema).unwrap();
    assert_eq!(demand.input_bytes(), value.encoded.len());
    let changed = payload(identity, "a finite model-shaped request", 13);
    let other_inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &changed,
    }];
    let changed_demand = CostDemand::admit_with_structured(&gear, &other_inputs, &schema).unwrap();
    assert_eq!(
        demand.validate_match(changed_demand),
        Err(CostEvidenceRefusal::WorkExtentChanged)
    );
}
#[test]
fn different_nominal_work_semantics_and_corrupt_encodings_are_refused() {
    let model = "fixture/model-request@1";
    let printer = "fixture/printer-request@1";
    let gear = gear(model);
    let schema = [CostStructuredInputSchema::admit(&request_type(model), 4096).unwrap()];
    // Same representation, distinct protocol meaning. Retagging the payload's
    // claimed Kind cannot change the canonical schema carried by the bytes.
    let mut foreign = payload(printer, "same fields", 12);
    foreign.value_kind = model.into();
    assert!(matches!(
        CostDemand::admit_with_structured(
            &gear,
            &[CostInput {
                port: &gear.inputs[0].port_id,
                value: &foreign
            }],
            &schema
        ),
        Err(CostEvidenceRefusal::Structured(
            StructuredInfoRefusal::WrongType
        ))
    ));
    let mut corrupt = payload(model, "same fields", 12);
    corrupt.encoded.push(0);
    assert!(matches!(
        CostDemand::admit_with_structured(
            &gear,
            &[CostInput {
                port: &gear.inputs[0].port_id,
                value: &corrupt
            }],
            &schema
        ),
        Err(CostEvidenceRefusal::Structured(_))
    ));
}
#[test]
fn catalogue_collisions_ambiguity_and_unadmitted_schema_extents_refuse() {
    let unknown_leaf = StructuredInfoType::nominal(
        "fixture/opaque-wrapper@1".into(),
        StructuredInfoType::leaf("fixture/unchecked-leaf@1".into()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        CostStructuredInputSchema::admit(&unknown_leaf, 4096),
        Err(CostEvidenceRefusal::InvalidWorkInput)
    ));
    assert!(matches!(
        CostStructuredInputSchema::admit(&request_type(COUNT_INFO_ID), 4096),
        Err(CostEvidenceRefusal::InvalidWorkInput)
    ));
    let identity = "fixture/model-request@1";
    assert!(matches!(
        CostStructuredInputSchema::admit(&request_type(identity), MAXIMUM_COST_INPUT_BYTES + 1),
        Err(CostEvidenceRefusal::InputLimit)
    ));
    let schemas = [
        CostStructuredInputSchema::admit(&request_type(identity), 4096).unwrap(),
        CostStructuredInputSchema::admit(&request_type(identity), 4096).unwrap(),
    ];
    assert!(matches!(
        CostDemand::admit_with_structured(&gear(identity), &[], &schemas),
        Err(CostEvidenceRefusal::AmbiguousEvidence)
    ));
}
fn window(host: &HostAdvertisement) -> CostObservationWindow {
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
        MonotonicInstant::new(200, clock).unwrap(),
    )
    .unwrap()
}
fn forecast(ms: i128) -> FulfillmentCostForecast {
    let mut work = ExactNumericWork::new(100_000_000);
    let duration = CostDuration::admit(
        RationalQuantity::new(
            ExactRational::from_i128(ms),
            Unit::resolve("ms").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut work,
    )
    .unwrap();
    let zero = CostDuration::admit(
        RationalQuantity::new(
            ExactRational::ZERO,
            Unit::resolve("s").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut work,
    )
    .unwrap();
    let point = CostDurationRange::admit(duration, duration, &mut work).unwrap();
    let zero = CostDurationRange::admit(zero, zero, &mut work).unwrap();
    FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: Some(zero),
            queue: Some(zero),
            service: Some(point),
            transfer: Some(zero),
        },
        &mut work,
    )
    .unwrap()
}
#[test]
fn fast_cost_cannot_admit_a_structured_input_larger_than_the_realization_queue() {
    let identity = "fixture/model-request@1";
    let gear = gear(identity);
    let schemas = [CostStructuredInputSchema::admit(&request_type(identity), 4096).unwrap()];
    let value = payload(identity, &"x".repeat(100), 12);
    let inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &value,
    }];
    let demand = CostDemand::admit_with_structured(&gear, &inputs, &schemas).unwrap();
    let mut hosts = common::competing_hosts();
    for host in &mut hosts {
        let offer = &mut host.capabilities[0];
        offer.kind_id = gear.kind_id.clone();
        offer.kind_contract_revision = gear.kind_contract_revision.clone();
        offer.startup_parameters = gear.startup_parameters.clone();
        offer.inputs = gear.inputs.clone();
        offer.outputs = gear.outputs.clone();
        offer.semantic_contract = gear.semantic_contract.clone();
        offer.implementation.implementation_id = "fixture/input-work@1".into();
        offer.implementation.artifact_id = "fixture/input-work-artifact@1".into();
        offer.limits.max_queue_bytes = 4096;
    }
    hosts[0].capabilities[0].limits.max_queue_bytes = value.encoded.len() as u32 - 1;
    let profile = ArtifactId::from("calibration/input-work@1");
    let revisions = [CostCalibrationRevision::new(&profile, 1).unwrap()];
    let signs: [SignId; 2] = ["forecast/too-small", "forecast/adequate"].map(Into::into);
    let sources = hosts
        .iter()
        .zip(&signs)
        .map(|(host, sign)| {
            CostEvidenceSource::bind(
                host,
                &host.capabilities[0],
                None,
                sign,
                revisions[0],
                window(host),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let forecasts = [forecast(1), forecast(100)];
    let samples = sources
        .iter()
        .zip(&forecasts)
        .map(|(source, forecast)| CostedForecast::bind(source, demand, forecast).unwrap())
        .collect::<Vec<_>>();
    let observations = common::resource_observations(&hosts);
    let timed = observations
        .iter()
        .map(|observation| {
            let host = hosts
                .iter()
                .find(|host| host.host_id == observation.host_id)
                .unwrap();
            TimedResourceObservation::admit(observation, window(host)).unwrap()
        })
        .collect::<Vec<_>>();
    let times = hosts
        .iter()
        .map(|host| MonotonicInstant::new(150, window(host).observed().clock().clone()).unwrap())
        .collect::<Vec<_>>();
    let contexts = hosts
        .iter()
        .zip(&times)
        .map(|(host, now)| CostEvaluationContext {
            host,
            calibrations: &revisions,
            now,
        })
        .collect::<Vec<_>>();
    let requirements = HardRealizationRequirements::default();
    let tie_policy = RealizationPolicy::default();
    let result = select_realization_with_costs(
        demand,
        CostSelectionInputs {
            hosts: &hosts,
            contexts: &contexts,
            advertisements: &[],
            requirements: &requirements,
            observations: &timed,
            tie_policy: &tie_policy,
            forecasts: &samples,
            intent: PerformanceIntent::Interactive,
            unavailable: UnavailableCostPolicy::PreferKnown,
        },
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    assert_eq!(result.realization.choice.host_id, hosts[1].host_id);
    assert_eq!(
        result.realization.signs[0].disposition,
        RealizationDecisionDisposition::Rejected(RealizationRejection::QueueByteBound)
    );
}
