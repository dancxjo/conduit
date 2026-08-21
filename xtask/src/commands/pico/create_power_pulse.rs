use std::time::Duration;

use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::doctor::repo_root;
use super::firmware::identity_manifest_path;
use super::serial::resolve_inert_port;
use super::{PicoArgs, PicoResult};

const REQUEST_PREFIX: &str = "CONDUIT_CREATE_POWER_PULSE@1:";
const RESPONSE_SCHEMA: &str = "conduit.netherwick/create-power-pulse@1";

pub fn run(args: &PicoArgs) -> PicoResult<()> {
    if !args.netherwick_inert {
        return Err("pico power-create requires --netherwick-inert".into());
    }
    let identity: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        identity_manifest_path(&repo_root()),
    )?)?;
    let expected_build = identity["firmware_build_id"]
        .as_str()
        .ok_or("inert image identity missing firmware_build_id; rebuild the image")?;
    if args.dry_run {
        println!("==> pico power-create (dry-run): one bounded 500 ms low-high-low pulse");
        return Ok(());
    }
    let port = resolve_inert_port(args.port.as_deref())?;
    println!(
        "==> pico power-create: requesting one bounded pulse from {}",
        port.display()
    );
    let mut line = NativePathCdcLine::open(&port, 1024)?;
    std::thread::sleep(Duration::from_millis(250));
    let request = format!("{REQUEST_PREFIX}{expected_build}");
    line.send_raw_stream_frame(request.as_bytes(), Duration::from_secs(2))?;
    let mut response = [0_u8; 1024];
    let response = line.receive_raw_stream_frame(&mut response, Duration::from_secs(3))?;
    let record: serde_json::Value = serde_json::from_slice(response)?;
    println!("{}", serde_json::to_string(&record)?);
    if record["schema"] != RESPONSE_SCHEMA
        || record["build_id"] != expected_build
        || record["success"] != true
        || record["pulse_ms"] != 500
        || record["translator_final"] != "low"
        || record["power_toggle_final"] != "low"
        || record["motion_opcode_sent"] != false
    {
        return Err(format!("Create power pulse refused or invalid: {record}").into());
    }
    println!("==> pico power-create: bounded pulse completed; outputs are low");
    Ok(())
}
