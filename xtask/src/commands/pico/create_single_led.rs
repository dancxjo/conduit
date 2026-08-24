use std::time::Duration;

use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::capstone_serial::resolve_port as resolve_capstone_port;
use super::doctor::repo_root;
use super::firmware::identity_manifest_path;
use super::{PicoArgs, PicoResult};

const REQUEST_PREFIX: &str = "CONDUIT_CREATE_SINGLE_LED@1:";
const RESPONSE_SCHEMA: &str = "conduit.pete/create-single-led-receipt@1";
const AUTHORITY_GRANT: &str = "grant/pete-create-single-led-no-motion-hil";

pub fn run(args: &PicoArgs, wheels_clear: bool) -> PicoResult<()> {
    if !args.pete_capstone {
        return Err("pico probe-create-led requires --pete-capstone".into());
    }
    if !wheels_clear {
        return Err(
            "single-LED probe refused: pass --wheels-clear only after the robot is stopped, attended, and its wheels cannot propel it"
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
            "==> pico probe-create-led (dry-run): START/FULL, one PLAY-only LED command [139,2,0,0], OE low for 60000 ms, START/SAFE cleanup; no music or motion authority"
        );
        return Ok(());
    }

    let port = resolve_capstone_port(args.port.as_deref())?;
    println!(
        "==> pico probe-create-led: requesting one PLAY-only LED command and isolated 60-second observation hold from {}",
        port.display()
    );
    let mut line = NativePathCdcLine::open(&port, 1024)?;
    std::thread::sleep(Duration::from_millis(250));
    let _ = line.discard_pending_raw_bytes()?;
    let request = format!("{REQUEST_PREFIX}{expected_build}:{AUTHORITY_GRANT}");
    line.send_raw_stream_frame(request.as_bytes(), Duration::from_secs(2))?;
    let mut response = [0_u8; 1024];
    let response = line.receive_raw_stream_frame(&mut response, Duration::from_secs(70))?;
    let record: serde_json::Value = serde_json::from_slice(response)?;
    println!("{}", serde_json::to_string(&record)?);
    validate_receipt(&record, expected_build)?;
    println!(
        "==> pico probe-create-led: bounded hold completed; SAFE commanded and translator low; physical LED visibility remains an operator observation"
    );
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
        || record["led_command"] != serde_json::json!([139, 2, 0, 0])
        || record["led_command_sent"] != true
        || record["requested_indicator"] != "play"
        || record["power_color"] != 0
        || record["power_intensity"] != 0
        || record["hold_ms"] != 60_000
        || record["hold_completed"] != true
        || record["translator_low_during_hold"] != true
        || record["mode_observed"] != false
        || record["physical_led_observed"] != false
        || record["safe_cleanup_command_sent"] != true
        || record["translator_final_level"] != "low"
        || record["uart_tx_bytes"] != 8
        || record["uart_rx_required"] != false
        || record["music_commands_sent"] != 0
        || record["motion_authority_granted"] != false
        || record["authority_grant_id"] != AUTHORITY_GRANT
    {
        return Err(format!("bounded Create single-LED probe refused or failed: {record}").into());
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
            "led_command": [139, 2, 0, 0],
            "led_command_sent": true,
            "requested_indicator": "play",
            "power_color": 0,
            "power_intensity": 0,
            "hold_ms": 60000,
            "hold_completed": true,
            "translator_low_during_hold": true,
            "mode_observed": false,
            "physical_led_observed": false,
            "safe_cleanup_command_sent": true,
            "translator_final_level": "low",
            "uart_tx_bytes": 8,
            "uart_rx_required": false,
            "music_commands_sent": 0,
            "motion_authority_granted": false,
            "authority_grant_id": AUTHORITY_GRANT,
        })
    }

    #[test]
    fn accepts_only_exact_play_led_isolated_hold_receipt() {
        assert!(validate_receipt(&valid_receipt(), "exact-build").is_ok());

        for (field, value) in [
            ("music_commands_sent", json!(1)),
            ("motion_authority_granted", json!(true)),
            ("translator_low_during_hold", json!(false)),
            ("safe_cleanup_command_sent", json!(false)),
            ("physical_led_observed", json!(true)),
        ] {
            let mut invalid = valid_receipt();
            invalid[field] = value;
            assert!(validate_receipt(&invalid, "exact-build").is_err());
        }
    }

    #[test]
    fn rejects_any_led_command_other_than_play_only() {
        for command in [json!([139, 2, 128, 255]), json!([139, 8, 0, 0])] {
            let mut invalid = valid_receipt();
            invalid["led_command"] = command;
            assert!(validate_receipt(&invalid, "exact-build").is_err());
        }
    }
}
