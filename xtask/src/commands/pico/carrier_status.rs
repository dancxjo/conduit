use std::time::Duration;

use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::doctor::repo_root;
use super::firmware::identity_manifest_path;
use super::serial::resolve_inert_port;
use super::{PicoArgs, PicoResult};

const REQUEST_PREFIX: &str = "CONDUIT_CARRIER_STATUS@1:";
const RESPONSE_SCHEMA: &str = "conduit.netherwick/carrier-status@1";

pub fn run(args: &PicoArgs) -> PicoResult<()> {
    if !args.netherwick_inert {
        return Err("pico carrier-status requires --netherwick-inert".into());
    }
    let identity: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        identity_manifest_path(&repo_root()),
    )?)?;
    let expected_build = identity["firmware_build_id"]
        .as_str()
        .ok_or("inert image identity missing firmware_build_id; rebuild the image")?;
    if args.dry_run {
        println!("==> pico carrier-status (dry-run): {expected_build}");
        return Ok(());
    }
    let port = resolve_inert_port(args.port.as_deref())?;
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
        || record["charging_indicator"]["gpio"] != 20
        || record["charging_indicator"]["active_high"] != true
        || !matches!(
            record["charging_indicator"]["level"].as_str(),
            Some("low" | "high")
        )
        || record["translator_oe"] != "low"
        || !record["create_probe_available"].is_boolean()
    {
        return Err(format!("invalid inert carrier status: {record}").into());
    }
    Ok(())
}
