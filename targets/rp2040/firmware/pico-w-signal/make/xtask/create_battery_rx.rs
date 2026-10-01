use std::time::Duration;

use conduit_create_oi::decode_sensor_packet;
use conduit_pete::lower_group_zero;
use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::capstone_serial::resolve_port as resolve_capstone_port;
use super::doctor::repo_root;
use super::firmware::identity_manifest_path;
use super::{PicoArgs, PicoResult};

const REQUEST_PREFIX: &str = "CONDUIT_CREATE_BATTERY_RX@1:";
const RESPONSE_SCHEMA: &str = "conduit.pete/create-battery-rx@1";
const AUTHORITY_GRANT: &str = "grant/pete-create-battery-rx-no-motion-hil";
const CONTROL_FRAME_MAX: usize = 768;

pub fn run(args: &PicoArgs, wheels_clear: bool) -> PicoResult<()> {
    if !args.pete_capstone {
        return Err("pico read-create-battery requires --pete-capstone".into());
    }
    if !wheels_clear {
        return Err(
            "battery RX probe refused: pass --wheels-clear only after the robot is stopped, attended, and its wheels cannot propel it"
                .into(),
        );
    }
    let identity: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        identity_manifest_path(&repo_root()),
    )?)?;
    let expected_build = identity["firmware_build_id"]
        .as_str()
        .ok_or("capstone image identity missing firmware_build_id; rebuild the image")?;
    if args.dry_run {
        println!(
            "==> pico read-create-battery (dry-run): START [128], one packet-0 query [142,0], exactly 26 RX bytes, START/SAFE [128,131], OE low; no Full, stream, retry, music, lights, or motion"
        );
        return Ok(());
    }

    let port = resolve_capstone_port(args.port.as_deref())?;
    println!(
        "==> pico read-create-battery: requesting one bounded Create OI packet-0 battery sample from {}",
        port.display()
    );
    let mut line = NativePathCdcLine::open(&port, 1024)?;
    std::thread::sleep(Duration::from_millis(250));
    let _ = line.discard_pending_raw_bytes()?;
    let request = format!("{REQUEST_PREFIX}{expected_build}:{AUTHORITY_GRANT}");
    line.send_raw_stream_frame(request.as_bytes(), Duration::from_secs(2))?;
    let mut response = [0_u8; 1024];
    let response = line.receive_raw_stream_frame(&mut response, Duration::from_secs(4))?;
    if response.len() > CONTROL_FRAME_MAX {
        return Err(format!(
            "Create battery receipt exceeds the {CONTROL_FRAME_MAX}-byte firmware control-frame bound"
        )
        .into());
    }
    let record: serde_json::Value = serde_json::from_slice(response)?;
    println!("{}", serde_json::to_string(&record)?);
    let battery = validate_receipt(&record, expected_build)?;
    println!(
        "==> Create battery RX: {} mV, {} mA, {} C, {}/{} mAh ({} permille), charging state {}",
        battery.millivolts,
        battery.milliamps,
        battery.temperature_celsius,
        battery.charge_mah,
        battery.capacity_mah,
        battery.charge_permille,
        battery.charging_state,
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BatteryState {
    charging_state: u8,
    millivolts: u16,
    milliamps: i16,
    temperature_celsius: i8,
    charge_mah: u16,
    capacity_mah: u16,
    charge_permille: u16,
}

fn validate_receipt(record: &serde_json::Value, expected_build: &str) -> PicoResult<BatteryState> {
    if record["schema"] != RESPONSE_SCHEMA
        || record["build_id"] != expected_build
        || record["success"] != true
        || record["state"] != "completed"
        || record["result_code"] != 0
        || record["start_sent"] != true
        || record["query"] != serde_json::json!([142, 0])
        || record["query_sent"] != true
        || record["prequery_discarded"] != 0
        || record["rx_bytes"] != 26
        || record["rx_outcome"] != "valid"
        || record["rx_valid"] != true
        || record["safe_sent"] != true
        || record["oe_final"] != "low"
        || record["uart_tx_bytes"] != 5
        || record["motion_authority"] != false
        || record["grant"] != AUTHORITY_GRANT
    {
        return Err(format!("bounded Create battery RX probe refused or failed: {record}").into());
    }

    let raw = decode_hex(
        record["rx_hex"]
            .as_str()
            .ok_or("battery receipt is missing exact RX hex")?,
    )?;
    let packet = decode_sensor_packet(0, &raw)
        .map_err(|error| format!("battery receipt contains invalid OI packet 0: {error:?}"))?;
    let group = lower_group_zero(&packet)
        .map_err(|error| format!("battery receipt cannot lower OI packet 0: {error:?}"))?;
    let portable = group
        .charging
        .battery()
        .map_err(|error| format!("battery receipt has inconsistent charge: {error:?}"))?
        .ok_or("Create reported no battery capacity")?;
    let expected = BatteryState {
        charging_state: raw[16],
        millivolts: group.charging.millivolts,
        milliamps: group.charging.milliamps,
        temperature_celsius: group.charging.temperature_celsius,
        charge_mah: group.charging.charge_mah,
        capacity_mah: group.charging.capacity_mah,
        charge_permille: portable.charge_permille(),
    };
    let reported = BatteryState {
        charging_state: json_u8(record, "charging_state")?,
        millivolts: json_u16(record, "millivolts")?,
        milliamps: json_i16(record, "milliamps")?,
        temperature_celsius: json_i8(record, "temperature_celsius")?,
        charge_mah: json_u16(record, "charge_mah")?,
        capacity_mah: json_u16(record, "capacity_mah")?,
        charge_permille: json_u16(record, "charge_permille")?,
    };
    if reported != expected {
        return Err(format!(
            "battery receipt fields do not match exact OI RX bytes: reported {reported:?}, decoded {expected:?}"
        )
        .into());
    }
    Ok(reported)
}

fn decode_hex(value: &str) -> PicoResult<Vec<u8>> {
    if value.len() != 52 || !value.len().is_multiple_of(2) {
        return Err("battery RX hex must encode exactly 26 bytes".into());
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let text = std::str::from_utf8(pair)?;
            Ok(u8::from_str_radix(text, 16)?)
        })
        .collect()
}

fn json_u8(record: &serde_json::Value, field: &str) -> PicoResult<u8> {
    Ok(u8::try_from(record[field].as_u64().ok_or_else(|| {
        format!("battery receipt field {field} is not an unsigned integer")
    })?)?)
}

fn json_u16(record: &serde_json::Value, field: &str) -> PicoResult<u16> {
    Ok(u16::try_from(record[field].as_u64().ok_or_else(|| {
        format!("battery receipt field {field} is not an unsigned integer")
    })?)?)
}

fn json_i8(record: &serde_json::Value, field: &str) -> PicoResult<i8> {
    Ok(i8::try_from(record[field].as_i64().ok_or_else(|| {
        format!("battery receipt field {field} is not an integer")
    })?)?)
}

fn json_i16(record: &serde_json::Value, field: &str) -> PicoResult<i16> {
    Ok(i16::try_from(record[field].as_i64().ok_or_else(|| {
        format!("battery receipt field {field} is not an integer")
    })?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SAMPLE_RX: &str = "000000000000000000000000000000000203e8ff9c1917701f40";

    fn valid_receipt() -> serde_json::Value {
        json!({
            "schema": RESPONSE_SCHEMA,
            "build_id": "exact-build",
            "success": true,
            "state": "completed",
            "result_code": 0,
            "start_sent": true,
            "query": [142, 0],
            "query_sent": true,
            "prequery_discarded": 0,
            "rx_bytes": 26,
            "rx_outcome": "valid",
            "rx_valid": true,
            "rx_hex": SAMPLE_RX,
            "charging_state": 2,
            "millivolts": 1000,
            "milliamps": -100,
            "temperature_celsius": 25,
            "charge_mah": 6000,
            "capacity_mah": 8000,
            "charge_permille": 750,
            "safe_sent": true,
            "oe_final": "low",
            "uart_tx_bytes": 5,
            "motion_authority": false,
            "grant": AUTHORITY_GRANT,
        })
    }

    #[test]
    fn accepts_only_fields_decoded_from_exact_rx_bytes() {
        let expected = BatteryState {
            charging_state: 2,
            millivolts: 1_000,
            milliamps: -100,
            temperature_celsius: 25,
            charge_mah: 6_000,
            capacity_mah: 8_000,
            charge_permille: 750,
        };
        assert_eq!(
            validate_receipt(&valid_receipt(), "exact-build").unwrap(),
            expected
        );

        let mut mismatch = valid_receipt();
        mismatch["millivolts"] = json!(999);
        assert!(validate_receipt(&mismatch, "exact-build").is_err());
    }

    #[test]
    fn rejects_corrupt_or_incomplete_rx() {
        for rx in [
            "",
            "00",
            "ff0000000000000000000000000000000203e8ff9c1917701f40",
        ] {
            let mut invalid = valid_receipt();
            invalid["rx_hex"] = json!(rx);
            assert!(validate_receipt(&invalid, "exact-build").is_err());
        }

        let mut inconsistent = valid_receipt();
        inconsistent["rx_hex"] = json!("000000000000000000000000000000000203e8ff9c1927101f40");
        assert!(validate_receipt(&inconsistent, "exact-build").is_err());
    }

    #[test]
    fn rejects_each_nonvalid_rx_outcome() {
        for (outcome, rx_bytes, rx) in [
            ("absent", 0, ""),
            ("truncated", 1, "00"),
            (
                "malformed",
                26,
                "ff0000000000000000000000000000000203e8ff9c1917701f40",
            ),
            (
                "inconsistent",
                26,
                "000000000000000000000000000000000203e8ff9c1927101f40",
            ),
        ] {
            let mut refused = valid_receipt();
            refused["success"] = json!(false);
            refused["state"] = json!("refused");
            refused["result_code"] = json!(7);
            refused["rx_outcome"] = json!(outcome);
            refused["rx_bytes"] = json!(rx_bytes);
            refused["rx_valid"] = json!(false);
            refused["rx_hex"] = json!(rx);
            assert!(validate_receipt(&refused, "exact-build").is_err());
        }
    }

    #[test]
    fn rejects_prequery_noise_even_with_a_valid_payload() {
        let mut noisy = valid_receipt();
        noisy["prequery_discarded"] = json!(1);
        assert!(validate_receipt(&noisy, "exact-build").is_err());
    }

    #[test]
    fn exact_receipt_fits_firmware_control_frame() {
        let mut receipt = valid_receipt();
        receipt["build_id"] = json!(
            "conduit-pico-w-pete-capstone:0123456789abcdef0123456789abcdef01234567:clean:thumbv6m-none-eabi:release:physical-play@1"
        );
        receipt["rx_hex"] = json!("0000000000000000000000000000000005ea60800080ffffffff");
        receipt["charging_state"] = json!(5);
        receipt["millivolts"] = json!(60_000);
        receipt["milliamps"] = json!(-32_768);
        receipt["temperature_celsius"] = json!(-128);
        receipt["charge_mah"] = json!(65_535);
        receipt["capacity_mah"] = json!(65_535);
        receipt["charge_permille"] = json!(1_000);
        assert!(validate_receipt(&receipt, receipt["build_id"].as_str().unwrap()).is_ok());
        let encoded = serde_json::to_vec(&receipt).unwrap();
        assert!(
            encoded.len() <= CONTROL_FRAME_MAX,
            "maximum valid receipt is {} bytes",
            encoded.len()
        );
    }
}
