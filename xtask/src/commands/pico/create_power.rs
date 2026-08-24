use std::time::Duration;

use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::capstone_serial::resolve_port as resolve_capstone_port;
use super::doctor::repo_root;
use super::firmware::identity_manifest_path;
use super::{PicoArgs, PicoResult};

const REQUEST_PREFIX: &str = "CONDUIT_WAKE_CREATE@1:";
const AUTHORITY_GRANT: &str = "grant/pete-pico-confirmed-off-wake-hil";
const ACCEPTED_SCHEMA: &str = "conduit.pete/create-power-pulse-accepted@1";
const RESPONSE_SCHEMA: &str = "conduit.pete/create-power-pulse@1";

pub fn run(args: &PicoArgs, confirmed_off: bool) -> PicoResult<()> {
    if !args.pete_capstone {
        return Err("pico wake-create requires --pete-capstone".into());
    }
    if !confirmed_off {
        return Err(
            "power toggle refused: pass --confirmed-off only after physically observing the Create powered off"
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
        println!("==> pico wake-create (dry-run): one confirmed-off low-high-low pulse");
        return Ok(());
    }
    let port = resolve_capstone_port(args.port.as_deref())?;
    println!(
        "==> pico wake-create: requesting one confirmed-off power pulse from {}",
        port.display()
    );
    let mut line = NativePathCdcLine::open(&port, 1024)?;
    std::thread::sleep(Duration::from_millis(250));
    let _ = line.discard_pending_raw_bytes()?;
    let request = format!("{REQUEST_PREFIX}{expected_build}:{AUTHORITY_GRANT}");
    line.send_raw_stream_frame(request.as_bytes(), Duration::from_secs(2))?;
    let mut accepted = [0_u8; 1024];
    let accepted = line.receive_raw_stream_frame(&mut accepted, Duration::from_secs(2))?;
    let accepted: serde_json::Value = serde_json::from_slice(accepted)?;
    validate_accepted(&accepted, expected_build)?;
    println!("{}", serde_json::to_string(&accepted)?);
    let mut response = [0_u8; 1024];
    let response = line.receive_raw_stream_frame(&mut response, Duration::from_secs(2))?;
    let record: serde_json::Value = serde_json::from_slice(response)?;
    println!("{}", serde_json::to_string(&record)?);
    validate_receipt(&record, expected_build).map_err(Into::into)
}

fn validate_accepted(record: &serde_json::Value, expected_build: &str) -> Result<(), String> {
    if record["schema"] != ACCEPTED_SCHEMA
        || record["build_id"] != expected_build
        || record["state"] != "accepted_low"
        || record["authority_grant_id"] != AUTHORITY_GRANT
        || record["gpio"] != 18
        || record["current_level"] != "low"
        || record["uart_enabled"] != false
        || record["motion_commanded"] != false
    {
        return Err(format!(
            "Create power pulse was not safely accepted: {record}"
        ));
    }
    Ok(())
}

fn validate_receipt(record: &serde_json::Value, expected_build: &str) -> Result<(), String> {
    if record["schema"] != RESPONSE_SCHEMA
        || record["build_id"] != expected_build
        || record["success"] != true
        || record["state"] != "completed_low"
        || record["authority_grant_id"] != AUTHORITY_GRANT
        || record["prior_power_state"] != "confirmed_off"
        || record["post_pulse_power_state"] != "awaiting_uart_verification"
        || record["gpio"] != 18
        || record["low_settle_ms"] != 5
        || record["high_pulse_ms"] != 500
        || record["final_level"] != "low"
        || record["uart_enabled"] != false
        || record["motion_commanded"] != false
    {
        return Err(format!("Create power pulse refused or failed: {record}"));
    }
    println!("==> pico wake-create: pulse completed low; verify power through fresh UART evidence");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> serde_json::Value {
        serde_json::json!({
            "schema": RESPONSE_SCHEMA,
            "build_id": "build",
            "success": true,
            "state": "completed_low",
            "authority_grant_id": AUTHORITY_GRANT,
            "prior_power_state": "confirmed_off",
            "post_pulse_power_state": "awaiting_uart_verification",
            "gpio": 18,
            "low_settle_ms": 5,
            "high_pulse_ms": 500,
            "final_level": "low",
            "uart_enabled": false,
            "motion_commanded": false,
        })
    }

    fn accepted() -> serde_json::Value {
        serde_json::json!({
            "schema": ACCEPTED_SCHEMA,
            "build_id": "build",
            "state": "accepted_low",
            "authority_grant_id": AUTHORITY_GRANT,
            "gpio": 18,
            "current_level": "low",
            "uart_enabled": false,
            "motion_commanded": false,
        })
    }

    #[test]
    fn exact_completed_low_receipt_is_accepted() {
        validate_accepted(&accepted(), "build").unwrap();
        validate_receipt(&receipt(), "build").unwrap();
    }

    #[test]
    fn high_final_level_is_never_accepted() {
        let mut receipt = receipt();
        receipt["final_level"] = "high".into();
        assert!(validate_receipt(&receipt, "build").is_err());
    }
}
