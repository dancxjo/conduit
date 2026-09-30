use std::time::Duration;

use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::capstone_serial::resolve_port as resolve_capstone_port;
use super::doctor::repo_root;
use super::firmware::identity_manifest_path;
use super::{PicoArgs, PicoResult};

const REQUEST_PREFIX: &str = "CONDUIT_CREATE_LIGHTS_STAGE@1:";
const RESPONSE_SCHEMA: &str = "conduit.pete/create-lights-stage-receipt@1";
const AUTHORITY_GRANT: &str = "grant/pete-create-lights-no-motion-hil";
const PATTERN: &str = "netherwick-healthy-lights@1";

pub fn run(args: &PicoArgs, wheels_clear: bool) -> PicoResult<()> {
    if !args.pete_capstone {
        return Err("pico lights-create requires --pete-capstone".into());
    }
    if !wheels_clear {
        return Err(
            "light-stage probe refused: pass --wheels-clear only after the robot is stopped and physically clear"
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
            "==> pico lights-create (dry-run): START/FULL, eight 800 ms PLAY/ADVANCE light steps, lights off, SAFE; no music or motion authority"
        );
        return Ok(());
    }

    let port = resolve_capstone_port(args.port.as_deref())?;
    println!(
        "==> pico lights-create: requesting one bounded light-only pattern from {}",
        port.display()
    );
    let mut line = NativePathCdcLine::open(&port, 1024)?;
    std::thread::sleep(Duration::from_millis(250));
    let _ = line.discard_pending_raw_bytes()?;
    let request = format!("{REQUEST_PREFIX}{expected_build}:{AUTHORITY_GRANT}");
    line.send_raw_stream_frame(request.as_bytes(), Duration::from_secs(2))?;
    let mut response = [0_u8; 1024];
    let response = line.receive_raw_stream_frame(&mut response, Duration::from_secs(10))?;
    let record: serde_json::Value = serde_json::from_slice(response)?;
    println!("{}", serde_json::to_string(&record)?);
    validate_receipt(&record, expected_build)?;
    println!(
        "==> pico lights-create: light pattern completed; lights off, SAFE commanded, translator low"
    );
    Ok(())
}

fn validate_receipt(record: &serde_json::Value, expected_build: &str) -> PicoResult<()> {
    if record["schema"] != RESPONSE_SCHEMA
        || record["build_id"] != expected_build
        || record["success"] != true
        || record["state"] != "completed"
        || record["result_code"] != 0
        || record["pattern"] != PATTERN
        || record["start_command_sent"] != true
        || record["full_command_sent"] != true
        || record["mode_observed"] != false
        || record["light_steps"] != 8
        || record["light_step_ms"] != 800
        || record["music_commands_sent"] != 0
        || record["lights_off_sent"] != true
        || record["safe_cleanup_command_sent"] != true
        || record["translator_final_level"] != "low"
        || record["motion_authority_granted"] != false
        || record["authority_grant_id"] != AUTHORITY_GRANT
    {
        return Err(format!("bounded Create light-stage probe refused or failed: {record}").into());
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
            "pattern": PATTERN,
            "start_command_sent": true,
            "full_command_sent": true,
            "mode_observed": false,
            "light_steps": 8,
            "light_step_ms": 800,
            "music_commands_sent": 0,
            "lights_off_sent": true,
            "safe_cleanup_command_sent": true,
            "translator_final_level": "low",
            "motion_authority_granted": false,
            "authority_grant_id": AUTHORITY_GRANT,
        })
    }

    #[test]
    fn accepts_only_complete_music_free_light_receipt() {
        assert!(validate_receipt(&valid_receipt(), "exact-build").is_ok());

        let mut music = valid_receipt();
        music["music_commands_sent"] = json!(1);
        assert!(validate_receipt(&music, "exact-build").is_err());
    }
}
