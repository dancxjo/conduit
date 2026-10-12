//! Project actual adapter-call measurements after execution, before release.
//! These intervals exclude queue wait, warmup, and session transport.
use conduit_std_host::AdmittedLocalModelPoolMember;
use serde_json::{json, Value};

pub(super) fn prepare(member: &AdmittedLocalModelPoolMember) -> Value {
    match member.prepare_completed_work() {
        Ok(receipt) => {
            let placement = receipt.realization();
            json!({
                "phase":"measured",
                "schema":"conduit.proof/native-completed-work@1",
                "scope":"adapter_call_only",
                "selection":receipt.selection(),
                "host_id":placement.host_id,
                "boot_id":placement.boot_id,
                "offer_generation":placement.offer_generation,
                "capability_id":placement.capability_id,
                "implementation_id":placement.implementation_id,
                "artifact_id":placement.artifact_id,
                "started":receipt.started(),
                "completed":receipt.completed(),
                "elapsed":receipt.elapsed(),
                "measured_time_enclosure":enclosure(receipt.started(), receipt.elapsed()),
                "input_bytes":receipt.input_bytes(),
                "output_bytes":receipt.output_bytes(),
                "input_digest":receipt.input_digest(),
                "terminal":format!("{:?}",receipt.terminal()),
            })
        }
        Err(refusal) => json!({"phase":"measurement-refused","refusal":format!("{refusal:?}")}),
    }
}

/// Two real reads from the owner's identified process clock, outside Play.
pub(super) fn clock(host: &conduit_std_host::StdHost) -> Result<Value, String> {
    use conduit_std_host::{ThreadTimer, TimerAdapter};
    let owner = host.advertisement();
    let mut timer = ThreadTimer;
    let received = timer
        .monotonic_observation(&owner.host_id, &owner.boot_id)
        .ok_or("owner clock receive unavailable")?;
    let sent = timer
        .monotonic_observation(&owner.host_id, &owner.boot_id)
        .ok_or("owner clock send unavailable")?;
    Ok(
        json!({"phase":"clock-observed","host_id":owner.host_id,"boot_id":owner.boot_id,
        "peer_receive":received,"peer_send":sent}),
    )
}

/// Measurement enclosure only; not a forecast or a confidence interval.
fn enclosure(
    started: &conduit_core::MonotonicInstant,
    elapsed: conduit_core::MonotonicDuration,
) -> Value {
    use conduit_core::*;
    let result = (|| -> Result<Value, String> {
        let error = started
            .clock()
            .resolution_ticks()
            .checked_add(started.clock().uncertainty_ticks())
            .and_then(|ticks| ticks.checked_mul(2))
            .ok_or("measurement uncertainty overflow")?;
        let upper = elapsed
            .ticks()
            .checked_add(error)
            .ok_or("measurement upper overflow")?;
        let denominator = match elapsed.scale() {
            TemporalScale::Seconds => 1,
            TemporalScale::Milliseconds => 1_000,
            TemporalScale::Microseconds => 1_000_000,
            TemporalScale::Nanoseconds => 1_000_000_000,
        };
        let mut work = ExactNumericWork::new(131_072);
        let mut quantity = |ticks: u64| -> Result<Value, String> {
            let coordinate = ExactRational::ratio(ticks.into(), denominator, &mut work)
                .map_err(|e| format!("measurement ratio: {e:?}"))?;
            let quantity = RationalQuantity::new(
                coordinate,
                Unit::resolve("s").map_err(|e| format!("measurement Unit: {e:?}"))?,
                QuantityRole::Linear,
            )
            .map_err(|e| format!("measurement Time: {e:?}"))?;
            let mut bytes = [0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
            let length = quantity
                .encode(&mut bytes)
                .map_err(|e| format!("measurement encoding: {e:?}"))?;
            Ok(json!({"value_kind":RATIONAL_QUANTITY_ID,"encoded":&bytes[..length]}))
        };
        let lower = quantity(elapsed.ticks().saturating_sub(error))?;
        let upper = quantity(upper)?;
        Ok(
            json!({"phase":"measured-time-enclosure","lower":lower,"upper":upper,
            "scope":"adapter_call_only","basis":"two endpoint clock resolution plus uncertainty",
            "numeric_work":work.spent()}),
        )
    })();
    match result {
        Ok(value) => value,
        Err(refusal) => json!({"phase":"measurement-enclosure-refused","refusal":refusal}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::*;
    fn measured(ticks: u64) -> Value {
        let clock = MonotonicClockIdentity::new(
            "host/test".into(),
            "boot/test".into(),
            "clock/test".into(),
            TemporalScale::Microseconds,
            1,
            1,
        )
        .unwrap();
        enclosure(
            &MonotonicInstant::new(100, clock).unwrap(),
            MonotonicDuration::new(ticks, TemporalScale::Microseconds),
        )
    }
    fn coordinate(value: &Value) -> ExactRational {
        let payload: ValuePayload = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(payload.value_kind.as_str(), RATIONAL_QUANTITY_ID);
        let quantity =
            RationalQuantity::decode(&payload.encoded, &mut ExactNumericWork::new(131_072))
                .unwrap();
        assert_eq!(quantity.unit(), Unit::resolve("s").unwrap());
        assert_eq!(quantity.role(), QuantityRole::Linear);
        quantity.coordinate()
    }
    #[test]
    fn zero_elapsed_retains_nonzero_clock_error() {
        let value = measured(0);
        assert_eq!(coordinate(&value["lower"]), ExactRational::ZERO);
        assert_eq!(
            coordinate(&value["upper"]),
            ExactRational::ratio(4, 1_000_000, &mut ExactNumericWork::new(131_072)).unwrap()
        );
    }
    #[test]
    fn overflowing_measurement_refuses_without_wrapping() {
        assert_eq!(measured(u64::MAX)["phase"], "measurement-enclosure-refused");
    }
}
