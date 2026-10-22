#![no_std]
//! Bounded read-only HTTP Look over committed semantic observation quantities.
//! The snapshot is publication material, not another sampling or storage loop.
use conduit_core::{validate_canonical_structured_value as validate, Quantity, Unit};
use core::{cell::RefCell, fmt::Write};
use critical_section::Mutex;

#[derive(Clone, Copy)]
struct Snapshot {
    temperature: Option<Quantity>,
    humidity: Option<Quantity>,
    sampled_ms: u64,
    sequence: u32,
    status: &'static str,
    ap_ready: bool,
}
extern crate alloc;
use alloc::boxed::Box;
const INITIAL: Snapshot = Snapshot {
    temperature: None,
    humidity: None,
    sampled_ms: 0,
    sequence: 0,
    status: "warming",
    ap_ready: false,
};
static SNAPSHOT: Mutex<RefCell<Option<Box<Snapshot>>>> = Mutex::new(RefCell::new(None));
/// Prepare the finite publication buffer before Play seals its allocator.
pub fn initialize() {
    critical_section::with(|cs| {
        let mut slot = SNAPSHOT.borrow(cs).borrow_mut();
        assert!(slot.is_none());
        *slot = Some(Box::new(INITIAL));
    });
}
fn snapshot() -> Snapshot {
    critical_section::with(|cs| {
        SNAPSHOT
            .borrow(cs)
            .borrow()
            .as_deref()
            .copied()
            .unwrap_or(INITIAL)
    })
}
pub fn set_ap_ready() {
    critical_section::with(|cs| {
        SNAPSHOT
            .borrow(cs)
            .borrow_mut()
            .as_deref_mut()
            .unwrap()
            .ap_ready = true
    });
}
pub fn ap_ready() -> bool {
    snapshot().ap_ready
}

pub fn publish(index: usize, bytes: &[u8], now: u64) {
    let mut observation = None;
    let mut status = [
        "invalid-payload",
        "timed-out",
        "provider-failed",
        "geometry-refused",
    ][index];
    if index == 0 {
        if let Ok(result) = validate(bytes) {
            if let Ok(Some(frame)) = result.variant_payload("observation") {
                let quantity = |name| {
                    let leaf = frame.record_field(name).ok()??;
                    Quantity::decode(leaf.value_node().get(5..)?).ok()
                };
                observation = quantity("temperature").zip(quantity("humidity"));
                if observation.is_some() {
                    status = "available";
                }
            } else if let Ok(Some(failure)) = result.variant_payload("unavailable") {
                for reason in [
                    "short-frame",
                    "malformed-length",
                    "checksum",
                    "invalid-payload",
                ] {
                    if matches!(failure.variant_payload(reason), Ok(Some(_))) {
                        status = reason;
                        break;
                    }
                }
            }
        }
    }
    critical_section::with(|cs| {
        let mut slot = SNAPSHOT.borrow(cs).borrow_mut();
        let s = slot.as_deref_mut().unwrap();
        s.sequence += 1;
        s.status = status;
        if let Some((temperature, humidity)) = observation {
            s.temperature = Some(temperature);
            s.humidity = Some(humidity);
            s.sampled_ms = now;
        }
    });
}

fn coordinate(value: Quantity, unit: Unit) -> Option<heapless::String<48>> {
    let (coefficient, exponent) = value.convert_to_unit(unit).ok()?;
    // DHT11 quantity Forms admit at most hundredths and bounded physical ranges.
    // Refuse unexpected numeric extents instead of truncating them.
    if !(-6..=6).contains(&exponent) {
        return None;
    }
    let mut text = heapless::String::<48>::new();
    if exponent >= 0 {
        write!(
            text,
            "{}",
            coefficient.checked_mul(10_i128.checked_pow(exponent as u32)?)?
        )
        .ok()?;
    } else {
        let scale = 10_u128.pow((-exponent) as u32);
        let magnitude = coefficient.unsigned_abs();
        if coefficient < 0 {
            text.push('-').ok()?;
        }
        write!(
            text,
            "{}.{:0width$}",
            magnitude / scale,
            magnitude % scale,
            width = (-exponent) as usize
        )
        .ok()?;
    }
    Some(text)
}

pub fn response(now: u64, json: bool) -> heapless::String<2048> {
    let s = snapshot();
    let age = now.saturating_sub(s.sampled_ms);
    let fresh = s.status == "available" && s.temperature.is_some() && age <= 30000;
    let temperature = s.temperature.and_then(|q| coordinate(q, Unit::Fahrenheit));
    let humidity = s.humidity.and_then(|q| coordinate(q, Unit::Percent));
    let mut body = heapless::String::<1536>::new();
    if json {
        let _ = write!(body, "{{\"sequence\":{},\"status\":\"{}\",\"fresh\":{},\"temperature_f\":{},\"relative_humidity_percent\":{},\"sampled_at_boot_ms\":{},\"age_ms\":{},\"ap_ready\":{}}}", s.sequence, s.status, fresh,
            temperature.as_ref().map_or("null",|v|v.as_str()), humidity.as_ref().map_or("null",|v|v.as_str()), s.sampled_ms, age, s.ap_ready);
    } else {
        let _ = write!(body, "<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><meta http-equiv=refresh content=5><title>Conduit Climate</title><style>body{{font:20px system-ui;max-width:34rem;margin:3rem auto;padding:1rem;background:#f5f4ee;color:#203c38}}strong{{font-size:3rem}}small{{display:block;margin-top:1rem}}</style><h1>Conduit Climate</h1><p>{}</p><p><strong>{} &deg;F</strong></p><p>{}% relative humidity</p><small>Sensor: {}. Reading age: {} seconds. Sample {}.</small><small>Refreshes every 5 seconds; samples every 15 seconds. Local access point; no internet connection.</small><small><a href=/reading>Reading as JSON</a></small></html>",
            if fresh { "Current reading" } else if temperature.is_some() { "Last known reading — not current" } else { "Waiting for a valid sensor reading" },
            temperature.as_ref().map_or("—",|v|v.as_str()), humidity.as_ref().map_or("—",|v|v.as_str()), s.status, age/1000, s.sequence);
    }
    let mut out = heapless::String::<2048>::new();
    write!(out, "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}", if json { "application/json" } else { "text/html; charset=utf-8" }, body.len(), body).unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body(response: &str) -> &str {
        response.split_once("\r\n\r\n").unwrap().1
    }
    #[test]
    fn quantities_freshness_failure_and_http_length() {
        let q = |text| Quantity::parse_plot_literal(text).unwrap();
        assert_eq!(
            coordinate(q("19°C"), Unit::Fahrenheit).unwrap().as_str(),
            "66.2"
        );
        assert_eq!(
            coordinate(q("-10°C"), Unit::Fahrenheit).unwrap().as_str(),
            "14"
        );
        assert_eq!(coordinate(q("55%"), Unit::Percent).unwrap().as_str(), "55");
        initialize();
        let warming = response(100, false);
        assert!(body(&warming).contains("Waiting for a valid sensor reading"));
        critical_section::with(|cs| {
            let mut slot = SNAPSHOT.borrow(cs).borrow_mut();
            let s = slot.as_deref_mut().unwrap();
            s.temperature = Some(q("19°C"));
            s.humidity = Some(q("55%"));
            s.sampled_ms = 1000;
            s.sequence = 1;
            s.status = "available";
        });
        set_ap_ready();
        let current = response(31000, true);
        let json: serde_json::Value = serde_json::from_str(body(&current)).unwrap();
        assert_eq!(json["temperature_f"], 66.2);
        assert_eq!(json["fresh"], true);
        assert_eq!(json["ap_ready"], true);
        let length: usize = current
            .split("Content-Length: ")
            .nth(1)
            .unwrap()
            .split("\r\n")
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(length, body(&current).len());
        let stale = response(31001, false);
        assert!(body(&stale).contains("Last known reading"));
        publish(1, &[], 32000);
        let failed = response(32000, true);
        let json: serde_json::Value = serde_json::from_str(body(&failed)).unwrap();
        assert_eq!(json["status"], "timed-out");
        assert_eq!(json["fresh"], false);
        assert_eq!(json["sampled_at_boot_ms"], 1000);
        assert_eq!(json["temperature_f"], 66.2);
    }
}
