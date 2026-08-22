use std::time::Duration;

use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::capstone_serial::resolve_port as resolve_capstone_port;
use super::doctor::repo_root;
use super::firmware::identity_manifest_path;
use super::{PicoArgs, PicoResult};

const REQUEST_PREFIX: &str = "CONDUIT_PLAY@1:";
const RESPONSE_SCHEMA: &str = "conduit.play/physical-receipt@1";
const CAPSTONE_FORM: &str = "pete-capstone";
const AUTHORITY_GRANT: &str = "grant/pete-pico-wheels-off-floor-hil";

pub fn run(args: &PicoArgs, wheels_off_floor: bool) -> PicoResult<()> {
    if !args.pete_capstone {
        return Err("pico drive-create requires --pete-capstone".into());
    }
    if !wheels_off_floor {
        return Err(
            "motion refused: pass --wheels-off-floor only after every drive wheel is securely clear of the floor"
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
            "==> pico drive-create (dry-run): 50 mm/s forward for 250 ms, mandatory local zero"
        );
        return Ok(());
    }
    let port = resolve_capstone_port(args.port.as_deref())?;
    println!(
        "==> pico drive-create: requesting one attended bounded semantic motion from {}",
        port.display()
    );
    let mut line = NativePathCdcLine::open(&port, 1024)?;
    std::thread::sleep(Duration::from_millis(250));
    let _ = line.discard_pending_raw_bytes()?;
    let request = format!("{REQUEST_PREFIX}{expected_build}:{CAPSTONE_FORM}:{AUTHORITY_GRANT}");
    line.send_raw_stream_frame(request.as_bytes(), Duration::from_secs(2))?;
    let mut response = [0_u8; 1024];
    let response = line.receive_raw_stream_frame(&mut response, Duration::from_secs(4))?;
    let record: serde_json::Value = serde_json::from_slice(response)?;
    println!("{}", serde_json::to_string(&record)?);
    if record["schema"] != RESPONSE_SCHEMA
        || record["build_id"] != expected_build
        || record["success"] != true
        || record["state"] != "completed"
        || record["form"] != CAPSTONE_FORM
        || record["kernel"] != "conduit-kernel"
        || record["oi_exposed"] != false
        || record["selected_linear_microunits"] != 100_000
        || record["linear_mm_s"] != 50
        || record["angular_mrad_s"] != 0
        || record["ttl_ms"] != 250
        || record["setup"] != "wheels-off-floor"
        || record["authority_grant_id"] != AUTHORITY_GRANT
        || record["kernel_decisions"]
            .as_u64()
            .is_none_or(|value| value == 0)
        || record["kernel_signs"]
            .as_u64()
            .is_none_or(|value| value == 0)
        || record["final_zero_confirmed"] != true
    {
        return Err(format!("bounded Create motion refused or failed: {record}").into());
    }
    println!("==> pico drive-create: bounded motion completed with exact local zero");
    Ok(())
}
