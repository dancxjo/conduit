use conduit_core::*;
use conduit_planner::resource_cost::*;
use serde_json::{json, Value};
pub type Error = Box<dyn std::error::Error>;
pub fn fail(e: impl core::fmt::Debug) -> Error {
    format!("{e:?}").into()
}
pub fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
#[cfg(target_os = "linux")]
pub fn tick() -> Result<u64, Error> {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) } != 0 {
        return Err("CLOCK_MONOTONIC unavailable".into());
    }
    u64::try_from(t.tv_sec)?
        .checked_mul(1_000_000_000)
        .and_then(|v| v.checked_add(u64::try_from(t.tv_nsec).ok()?))
        .ok_or_else(|| "clock overflow".into())
}
#[cfg(target_os = "linux")]
pub fn identity(host: HostId, boot: BootId) -> Result<MonotonicClockIdentity, Error> {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_getres(libc::CLOCK_MONOTONIC, &mut t) } != 0 {
        return Err("clock resolution unavailable".into());
    }
    let resolution = u64::try_from(t.tv_sec)?
        .checked_mul(1_000_000_000)
        .and_then(|v| v.checked_add(u64::try_from(t.tv_nsec).ok()?))
        .ok_or("resolution overflow")?;
    MonotonicClockIdentity::new(
        host,
        boot,
        "linux/CLOCK_MONOTONIC".into(),
        TemporalScale::Nanoseconds,
        resolution,
        0,
    )
    .map_err(fail)
}
pub fn now(clock: &MonotonicClockIdentity) -> Result<MonotonicInstant, Error> {
    MonotonicInstant::new(tick()?, clock.clone()).map_err(fail)
}
pub fn measure<T>(
    clock: &MonotonicClockIdentity,
    f: impl FnOnce() -> Result<T, Error>,
) -> Result<(T, CostMeasuredDuration), Error> {
    let start = now(clock)?;
    let value = f()?;
    let end = now(clock)?;
    Ok((
        value,
        CostMeasuredDuration::measure(&start, &end, &mut work()).map_err(fail)?,
    ))
}
pub fn duration(ns: u64) -> Result<CostDuration, Error> {
    CostDuration::admit(
        RationalQuantity::new(
            ExactRational::ratio(ns.into(), 1_000_000_000, &mut work()).map_err(fail)?,
            Unit::resolve("s").unwrap(),
            QuantityRole::Linear,
        )
        .map_err(fail)?,
        &mut work(),
    )
    .map_err(fail)
}
pub fn ratio(v: ExactRational) -> Result<Value, Error> {
    let negative = v.is_negative();
    let (n, d) = if negative { v.negated() } else { v }
        .checked_unsigned_ratio()
        .map_err(fail)?;
    Ok(
        json!({"numerator":format!("{}{n}",if negative {"-"} else {""}),"denominator":d.to_string()}),
    )
}
pub fn interval(r: CostDurationRange) -> Result<Value, Error> {
    Ok(json!({"lower":ratio(r.lower().seconds())?,"upper":ratio(r.upper().seconds())?,"unit":"s"}))
}
pub fn observation(m: &CostMeasuredDuration) -> Result<Value, Error> {
    Ok(
        json!({"start":m.started().ticks().to_string(),"end":m.completed().ticks().to_string(),"range":interval(m.range())?,"clock":m.completed().clock()}),
    )
}

#[cfg(not(target_os = "linux"))]
pub fn tick() -> Result<u64, Error> {
    Err("Linux CLOCK_MONOTONIC required".into())
}
#[cfg(not(target_os = "linux"))]
pub fn identity(_: HostId, _: BootId) -> Result<MonotonicClockIdentity, Error> {
    Err("Linux CLOCK_MONOTONIC required".into())
}
