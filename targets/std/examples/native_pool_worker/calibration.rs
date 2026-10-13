//! Optional adapter-service-only calibration; never a whole-fulfillment model.
mod history;
mod query;
use conduit_core::*;
use conduit_planner::resource_cost::*;
use conduit_std_host::{AdmittedLocalModelPoolMember, StdHost, ThreadTimer, TimerAdapter};
pub(super) use history::Retained;
pub(super) use query::query;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Estimate {
    lower: ValuePayload,
    upper: ValuePayload,
}
pub(super) struct Prepared {
    forecast: FulfillmentCostForecast,
    window: CostObservationWindow,
    origin: Option<SignId>,
    source: Option<CostSourceAttribution>,
}
fn quantity(value: &ValuePayload, work: &mut ExactNumericWork) -> Result<RationalQuantity, String> {
    if value.value_kind.as_str() != RATIONAL_QUANTITY_ID {
        return Err("service estimate semantic type changed".into());
    }
    RationalQuantity::decode(&value.encoded, work).map_err(|e| format!("service estimate: {e:?}"))
}
pub(super) fn prepare(
    host: &StdHost,
    estimate: Option<&Estimate>,
    reuse: bool,
    retained: Option<&Retained>,
    demand: CostDemand<'_>,
) -> Result<Option<Prepared>, String> {
    if reuse {
        if estimate.is_some() {
            return Err("service estimate and retained calibration are mutually exclusive".into());
        }
        return retained
            .ok_or("retained service calibration absent")?
            .prepare(host, demand)
            .map(Some);
    }
    let Some(estimate) = estimate else {
        return Ok(None);
    };
    let mut work = ExactNumericWork::new(100_000_000);
    let lower = CostDuration::admit(quantity(&estimate.lower, &mut work)?, &mut work)
        .map_err(|e| format!("service lower: {e:?}"))?;
    let upper = CostDuration::admit(quantity(&estimate.upper, &mut work)?, &mut work)
        .map_err(|e| format!("service upper: {e:?}"))?;
    let service = CostDurationRange::admit(lower, upper, &mut work)
        .map_err(|e| format!("service interval: {e:?}"))?;
    let zero = CostDuration::admit(
        RationalQuantity::new(
            ExactRational::ZERO,
            Unit::resolve("s").unwrap(),
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut work,
    )
    .map_err(|e| format!("service zero: {e:?}"))?;
    let zero = CostDurationRange::admit(zero, zero, &mut work)
        .map_err(|e| format!("service zero range: {e:?}"))?;
    // Zero non-service stages define this adapter-only model; they are not
    // observations that queue, setup or transfer in a request cost nothing.
    let forecast = FulfillmentCostForecast::sequential(
        CostForecastStages {
            setup: Some(zero),
            queue: Some(zero),
            service: Some(service),
            transfer: Some(zero),
        },
        &mut work,
    )
    .map_err(|e| format!("service model: {e:?}"))?;
    let owner = host.advertisement();
    let observed = ThreadTimer
        .monotonic_observation(&owner.host_id, &owner.boot_id)
        .ok_or("service forecast clock unavailable")?;
    let expires = MonotonicInstant::new(
        observed
            .ticks()
            .checked_add(1_000_000)
            .ok_or("service expiry overflow")?,
        observed.clock().clone(),
    )
    .map_err(|e| format!("service expiry: {e:?}"))?;
    let window = CostObservationWindow::admit(observed, expires)
        .map_err(|e| format!("service window: {e:?}"))?;
    Ok(Some(Prepared {
        forecast,
        window,
        origin: None,
        source: None,
    }))
}
fn encoded(quantity: RationalQuantity) -> Result<Value, String> {
    let mut bytes = [0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
    let length = quantity
        .encode(&mut bytes)
        .map_err(|e| format!("calibration encode: {e:?}"))?;
    Ok(json!({"value_kind":RATIONAL_QUANTITY_ID,"encoded":&bytes[..length]}))
}
pub(super) fn finish(
    host: &StdHost,
    member: &AdmittedLocalModelPoolMember,
    demand: CostDemand<'_>,
    prepared: Option<&Prepared>,
    retained: &mut Option<Retained>,
) -> Value {
    let Some(prepared) = prepared else {
        return json!({"phase":"service-calibration-not-requested"});
    };
    let result = (|| -> Result<Value, String> {
        let receipt = member
            .prepare_completed_work()
            .map_err(|e| format!("completion: {e:?}"))?;
        let owner = host.advertisement();
        let offer = owner
            .capabilities
            .iter()
            .find(|o| o.capability_id == receipt.realization().capability_id)
            .ok_or("calibration offer absent")?;
        let metric = ArtifactId::from("conduit.proof/adapter-service-calibration@1");
        let revision = CostCalibrationRevision::new(&metric, 1)
            .map_err(|e| format!("calibration revision: {e:?}"))?;
        let sign = |label: &str| {
            SignId::from(format!(
                "{label}/{}",
                receipt.selection().operation_id.as_str()
            ))
        };
        let forecast_sign = prepared
            .origin
            .clone()
            .unwrap_or_else(|| sign("service-forecast"));
        let completion_sign = sign("service-completion");
        let source = CostEvidenceSource::bind(
            owner,
            offer,
            None,
            &forecast_sign,
            revision,
            prepared.window.clone(),
        )
        .map_err(|e| format!("forecast source: {e:?}"))?;
        let expires = MonotonicInstant::new(
            receipt
                .completed()
                .ticks()
                .checked_add(1_000_000)
                .ok_or("completion expiry overflow")?,
            receipt.completed().clock().clone(),
        )
        .map_err(|e| format!("completion expiry: {e:?}"))?;
        let window = CostObservationWindow::admit(receipt.completed().clone(), expires)
            .map_err(|e| format!("completion window: {e:?}"))?;
        let completion =
            CostEvidenceSource::bind(owner, offer, None, &completion_sign, revision, window)
                .map_err(|e| format!("completion source: {e:?}"))?;
        let forecast = CostedForecast::bind(&source, demand, &prepared.forecast)
            .map_err(|e| format!("request forecast: {e:?}"))?;
        let mut work = ExactNumericWork::new(100_000_000);
        let calibrated = receipt
            .prepare_calibration(&forecast, &completion, None, &mut work)
            .map_err(|e| format!("native calibration: {e:?}"))?;
        let record = calibrated.record();
        let mut model = BoundedCostCalibration::admit(CostCalibrationPolicy {
            history_capacity: 1,
            maximum_updates: 1,
            minimum_update_ticks: 1,
        })
        .map_err(|e| format!("bounded calibration: {e:?}"))?;
        model
            .update(record, &mut work)
            .map_err(|e| format!("calibration update: {e:?}"))?;
        let estimated_at = ThreadTimer
            .monotonic_observation(&owner.host_id, &owner.boot_id)
            .ok_or("calibration snapshot clock unavailable")?;
        let estimate = model
            .snapshot_at(owner, revision, &estimated_at)
            .map_err(|e| format!("calibration snapshot: {e:?}"))?
            .ok_or("calibration estimate absent")?;
        estimate
            .validate_use(
                &completion,
                demand,
                CostExecutionCondition::Unknown,
                &estimated_at,
            )
            .map_err(|e| format!("calibration estimate use: {e:?}"))?;
        let next = CostedForecast::bind(&completion, demand, &prepared.forecast)
            .and_then(|forecast| {
                forecast.with_calibration(&estimate, CostExecutionCondition::Unknown, &estimated_at)
            })
            .map_err(|e| format!("calibrated next service forecast: {e:?}"))?;
        let expired = completion.window().expires();
        if estimate
            .validate_use(
                &completion,
                demand,
                CostExecutionCondition::Unknown,
                expired,
            )
            .is_ok()
        {
            return Err("calibration estimate unexpectedly admitted at expiry".into());
        }
        let (lower_error, upper_error) = record.error_bounds();
        let predicted = prepared
            .forecast
            .stages()
            .service
            .ok_or("service forecast absent")?;
        let value = json!({"phase":"adapter-service-calibrated","scope":"adapter_call_only","condition":"Unknown","updates":model.updates(),"schema":"conduit.proof/adapter-service-calibration@1",
                "estimated_at":estimated_at,
                "estimate_lower":encoded(estimate.duration().lower().original())?,
                "estimate_upper":encoded(estimate.duration().upper().original())?,
                "next_service_lower":encoded(seconds_quantity(next.lower_seconds())?)?,
                "next_service_upper":encoded(seconds_quantity(next.upper_seconds())?)?,
                "expiry_refusal_checked":true,
                "estimate_claim":"bounded mean estimate, not a future-outcome confidence bound",
                "selection":receipt.selection(),"input_digest":receipt.input_digest(),
                "forecast_source":attribution(&source),"completion_source":attribution(&completion),
                "predicted_lower":encoded(predicted.lower().original())?,"predicted_upper":encoded(predicted.upper().original())?,"measured_lower":encoded(record.measured().lower().original())?,"measured_upper":encoded(record.measured().upper().original())?,"error_lower":encoded(lower_error)?,"error_upper":encoded(upper_error)?,"numeric_work":work.spent()});
        *retained = Some(Retained::capture(
            owner,
            receipt,
            demand,
            estimate.duration(),
            completion.window().clone(),
            prepared.forecast.stages(),
        )?);
        Ok(value)
    })();
    match result {
        Ok(value) => value,
        Err(refusal) => json!({"phase":"service-calibration-refused","refusal":refusal}),
    }
}

fn attribution(source: &CostEvidenceSource<'_>) -> Value {
    attribution_owned(&CostSourceAttribution::from_source(source))
}
fn attribution_owned(value: &CostSourceAttribution) -> Value {
    json!({"host_id":value.host_id,"boot_id":value.boot_id,"offer_generation":value.offer_generation,
        "host_profile":value.host_profile,"capability_id":value.capability_id,"kind_id":value.kind_id,
        "kind_contract_revision":value.kind_contract_revision,"execution_profile_id":value.execution_profile_id,
        "implementation_id":value.implementation_id,"artifact_id":value.implementation_artifact,
        "resource":value.resource,"sign_id":value.sign_id,"calibration_identity":value.calibration_identity,
        "calibration_revision":value.calibration_revision,"observed":value.window.observed(),"expires":value.window.expires()})
}

fn seconds_quantity(value: ExactRational) -> Result<RationalQuantity, String> {
    RationalQuantity::new(
        value,
        Unit::resolve("s").map_err(|e| format!("forecast Unit: {e:?}"))?,
        QuantityRole::Linear,
    )
    .map_err(|e| format!("forecast Time: {e:?}"))
}
