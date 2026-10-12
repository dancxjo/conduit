//! Internal proof convenience for finite captured clock boundaries.
//! This checks Time projection, not producer admission or forecast calibration.
use conduit_core::{
    ExactNumericWork, MonotonicInstant, RationalQuantity, ValuePayload, RATIONAL_QUANTITY_ID,
    RATIONAL_QUANTITY_MAX_ENCODED_BYTES,
};
use conduit_planner::resource_cost::CostMeasuredDuration;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Boundary {
    phase: String,
    instant: MonotonicInstant,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    boundaries: Vec<Boundary>,
}
fn encoded(quantity: RationalQuantity) -> Result<ValuePayload, String> {
    let mut bytes = [0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
    let length = quantity
        .encode(&mut bytes)
        .map_err(|e| format!("Time encode: {e:?}"))?;
    Ok(ValuePayload {
        value_kind: RATIONAL_QUANTITY_ID.into(),
        encoded: bytes[..length].to_vec(),
    })
}
fn measurement(measured: &CostMeasuredDuration) -> Result<Value, String> {
    Ok(json!({
        "started":measured.started(), "completed":measured.completed(),
        "elapsed":measured.elapsed(),
        "lower":encoded(measured.range().lower().original())?,
        "upper":encoded(measured.range().upper().original())?,
        "scope":"clock endpoint measurement, not a future-outcome bound"
    }))
}
fn run() -> Result<(), String> {
    let mut raw = Vec::new();
    std::io::stdin()
        .take(65_537)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    if raw.len() > 65_536 {
        return Err("measurement input extent refused".into());
    }
    let input: Input = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    if !(2..=16).contains(&input.boundaries.len()) {
        return Err("measurement boundary count refused".into());
    }
    for (index, boundary) in input.boundaries.iter().enumerate() {
        if boundary.phase.is_empty()
            || boundary.phase.len() > 256
            || input.boundaries[..index]
                .iter()
                .any(|b| b.phase == boundary.phase)
        {
            return Err("measurement phase identity refused".into());
        }
    }
    let mut work = ExactNumericWork::new(100_000_000);
    let mut intervals = Vec::with_capacity(input.boundaries.len() - 1);
    let mut summed_ticks = 0_u64;
    for pair in input.boundaries.windows(2) {
        let measured = CostMeasuredDuration::measure(&pair[0].instant, &pair[1].instant, &mut work)
            .map_err(|e| format!("measurement: {e:?}"))?;
        summed_ticks = summed_ticks
            .checked_add(measured.elapsed().ticks())
            .ok_or("measurement sum overflow")?;
        intervals.push(
            json!({"from":pair[0].phase,"to":pair[1].phase,"measurement":measurement(&measured)?}),
        );
    }
    let whole = CostMeasuredDuration::measure(
        &input.boundaries[0].instant,
        &input.boundaries.last().unwrap().instant,
        &mut work,
    )
    .map_err(|e| format!("whole measurement: {e:?}"))?;
    if summed_ticks != whole.elapsed().ticks() {
        return Err("measurement partition changed".into());
    }
    let result = json!({"phase":"checked-cost-time-measurements","intervals":intervals,
        "whole":measurement(&whole)?, "numeric_work":work.spent(),
        "scope":"caller-supplied identified boundaries; no producer authentication, request binding or forecast admission"});
    serde_json::to_writer(std::io::stdout().lock(), &result).map_err(|e| e.to_string())?;
    std::io::stdout()
        .write_all(b"\n")
        .map_err(|e| e.to_string())
}
fn main() {
    if let Err(refusal) = run() {
        eprintln!("cost measurement refused: {refusal}");
        std::process::exit(1);
    }
}
