use std::time::Duration;

use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::capstone_serial::resolve_port as resolve_capstone_port;
use super::doctor::repo_root;
use super::firmware::identity_manifest_path;
use super::{PicoArgs, PicoResult};

const REQUEST_PREFIX: &str = "CONDUIT_CREATE_FULL_STAGE@1:";
const RESPONSE_SCHEMA: &str = "conduit.pete/create-full-stage-receipt@1";
const AUTHORITY_GRANT: &str = "grant/pete-create-full-no-motion-hil";

pub fn run(args: &PicoArgs, wheels_clear: bool) -> PicoResult<()> {
    if !args.pete_capstone {
        return Err("pico full-create requires --pete-capstone".into());
    }
    if !wheels_clear {
        return Err(
            "Full-stage probe refused: pass --wheels-clear only after the robot is stopped and physically clear"
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
            "==> pico full-create (dry-run): START, FULL, 500 ms hold, SAFE; four UART bytes and no RX or motion authority"
        );
        return Ok(());
    }

    let port = resolve_capstone_port(args.port.as_deref())?;
    println!(
        "==> pico full-create: requesting one bounded transmit-only Full-stage probe from {}",
        port.display()
    );
    let mut line = NativePathCdcLine::open(&port, 1024)?;
    std::thread::sleep(Duration::from_millis(250));
    let _ = line.discard_pending_raw_bytes()?;
    let request = format!("{REQUEST_PREFIX}{expected_build}:{AUTHORITY_GRANT}");
    line.send_raw_stream_frame(request.as_bytes(), Duration::from_secs(2))?;
    let mut response = [0_u8; 1024];
    let response = line.receive_raw_stream_frame(&mut response, Duration::from_secs(3))?;
    let record: serde_json::Value = serde_json::from_slice(response)?;
    println!("{}", serde_json::to_string(&record)?);
    validate_receipt(&record, expected_build)?;
    println!("==> pico full-create: START/FULL sent; SAFE commanded and translator returned low");
    Ok(())
}

fn validate_receipt(record: &serde_json::Value, expected_build: &str) -> PicoResult<()> {
    if record["schema"] != RESPONSE_SCHEMA
        || record["build_id"] != expected_build
        || record["success"] != true
        || record["state"] != "completed"
        || record["result_code"] != 0
        || record["start_command_sent"] != true
        || record["full_command_sent"] != true
        || record["full_hold_ms"] != 500
        || record["mode_observed"] != false
        || record["safe_cleanup_command_sent"] != true
        || record["translator_final_level"] != "low"
        || record["uart_tx_bytes"] != 4
        || record["uart_rx_required"] != false
        || record["motion_authority_granted"] != false
        || record["authority_grant_id"] != AUTHORITY_GRANT
    {
        return Err(format!("bounded Create Full-stage probe refused or failed: {record}").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_receipt() -> serde_json::Value {
        json!({
            "schema": RESPONSE_SCHEMA,
            "build_id": "exact-build",
            "success": true,
            "generation": 1,
            "state": "completed",
            "result_code": 0,
            "start_command_sent": true,
            "full_command_sent": true,
            "full_hold_ms": 500,
            "mode_observed": false,
            "safe_cleanup_command_sent": true,
            "translator_final_level": "low",
            "uart_tx_bytes": 4,
            "uart_rx_required": false,
            "motion_authority_granted": false,
            "authority_grant_id": AUTHORITY_GRANT,
        })
    }

    #[test]
    fn accepts_only_exact_safe_cleanup_receipt() {
        assert!(validate_receipt(&valid_receipt(), "exact-build").is_ok());

        let mut unsafe_receipt = valid_receipt();
        unsafe_receipt["safe_cleanup_command_sent"] = json!(false);
        assert!(validate_receipt(&unsafe_receipt, "exact-build").is_err());
    }
}
