//! Refresh owned producer evidence after forecast preparation. The caller must
//! actually timestamp the preparation receipt and then query its admitted pipes.
//! Serialized evidence alone provides no clock or producer authentication.
use conduit_core::*;
use serde::Deserialize;
use serde_json::json;
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Refresh {
    pub preparation_received_at: MonotonicInstant,
    pub admission: super::dispatch::Admission,
    pub samples: Vec<super::samples::Request>,
}
pub(super) fn request(reader: &mut impl Read, writer: &mut impl Write) -> Result<Refresh, String> {
    let notice=serde_json::to_vec(&json!({"phase":"native-dispatch-evaluation-required",
        "requirement":"timestamp this receipt, then capture new owned-pipe samples and clock exchanges"}))
        .map_err(|_| "evaluation notice invalid")?;
    writer
        .write_all(&(notice.len() as u32).to_be_bytes())
        .and_then(|_| writer.write_all(&notice))
        .and_then(|_| writer.flush())
        .map_err(|_| "evaluation notice unavailable")?;
    let mut prefix = [0; 4];
    reader
        .read_exact(&mut prefix)
        .map_err(|_| "evaluation header unavailable")?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > 1024 * 1024 {
        return Err("evaluation document extent refused".into());
    }
    let mut bytes = vec![0; length];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| "evaluation document incomplete")?;
    let refresh: Refresh =
        serde_json::from_slice(&bytes).map_err(|_| "evaluation document invalid")?;
    if refresh.samples.len() > 2 {
        return Err("evaluation source bound refused".into());
    }
    Ok(refresh)
}
pub(super) fn validate(
    preparation_received_at: &MonotonicInstant,
    initial: &super::samples::CheckedSample<'_>,
    current: &super::samples::CheckedSample<'_>,
) -> Result<(), String> {
    let initial_now = initial.current_local_sample()?;
    let query_started = current.current_query_started()?;
    validate_order(preparation_received_at, initial_now, query_started)?;
    if current.source().host() != initial.source().host()
        || current.source().offer() != initial.source().offer()
        || current.source().resource() != initial.source().resource()
        || current.source().calibration() != initial.source().calibration()
        || current.source().window().observed().ticks()
            <= initial.source().window().observed().ticks()
    {
        return Err("dispatch refresh changed scope or predates preparation receipt".into());
    }
    Ok(())
}

pub(super) fn validate_evaluation(
    evaluated_at: &MonotonicInstant,
    sample_now: &MonotonicInstant,
) -> Result<(), String> {
    if evaluated_at != sample_now {
        return Err("dispatch sample was not projected at evaluation instant".into());
    }
    Ok(())
}

fn validate_order(
    received: &MonotonicInstant,
    initial: &MonotonicInstant,
    query: &MonotonicInstant,
) -> Result<(), String> {
    if received.clock() != initial.clock()
        || received.ticks() < initial.ticks()
        || query.clock() != received.clock()
        || query.ticks() < received.ticks()
    {
        return Err("dispatch query predates preparation receipt or changes clock".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn clock(host: &str) -> MonotonicClockIdentity {
        MonotonicClockIdentity::new(
            HostId::from(host),
            BootId::from(format!("boot/{host}")),
            format!("clock/{host}"),
            TemporalScale::Microseconds,
            1,
            1,
        )
        .unwrap()
    }
    #[test]
    fn sample_projection_must_match_actual_evaluation() {
        let c = clock("coordinator");
        let evaluated = MonotonicInstant::new(200, c.clone()).unwrap();
        let earlier = MonotonicInstant::new(199, c.clone()).unwrap();
        let later = MonotonicInstant::new(201, c).unwrap();
        assert!(validate_evaluation(&evaluated, &earlier).is_err());
        assert!(validate_evaluation(&evaluated, &later).is_err());
        assert!(validate_evaluation(&evaluated, &evaluated).is_ok());
        let foreign = MonotonicInstant::new(200, clock("foreign")).unwrap();
        assert!(validate_evaluation(&evaluated, &foreign).is_err());
    }
    #[test]
    fn query_started_before_preparation_receipt_refuses() {
        let c = clock("coordinator");
        let instant = |ticks| MonotonicInstant::new(ticks, c.clone()).unwrap();
        assert!(validate_order(&instant(200), &instant(100), &instant(199)).is_err());
        assert!(validate_order(&instant(200), &instant(100), &instant(201)).is_ok());
    }
    #[test]
    fn old_preparation_receipt_cannot_renew_evidence() {
        let c = clock("coordinator");
        let instant = |ticks| MonotonicInstant::new(ticks, c.clone()).unwrap();
        assert!(validate_order(&instant(99), &instant(100), &instant(201)).is_err());
    }
    #[test]
    fn foreign_query_clock_refuses() {
        let c = clock("coordinator");
        let instant = |ticks| MonotonicInstant::new(ticks, c.clone()).unwrap();
        let foreign = MonotonicInstant::new(201, clock("foreign")).unwrap();
        assert!(validate_order(&instant(200), &instant(100), &foreign).is_err());
    }
}
