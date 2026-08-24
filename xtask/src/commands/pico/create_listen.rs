use std::time::Duration;

use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::capstone_serial::resolve_port as resolve_capstone_port;
use super::doctor::repo_root;
use super::firmware::identity_manifest_path;
use super::{PicoArgs, PicoResult};

const REQUEST_PREFIX: &str = "CONDUIT_CREATE_RX_LISTEN@1:";
const RESPONSE_SCHEMA: &str = "conduit.pete/create-rx-listen@1";

pub fn run(args: &PicoArgs) -> PicoResult<()> {
    if !args.pete_capstone {
        return Err("pico listen-create requires --pete-capstone".into());
    }
    let identity: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        identity_manifest_path(&repo_root()),
    )?)?;
    let expected_build = identity["firmware_build_id"]
        .as_str()
        .ok_or("capstone image identity missing firmware_build_id; rebuild the image")?;
    if args.dry_run {
        println!("==> pico listen-create (dry-run): OE high for 1000 ms, UART TX exactly zero");
        return Ok(());
    }
    let port = resolve_capstone_port(args.port.as_deref())?;
    let mut line = NativePathCdcLine::open(&port, 1024)?;
    std::thread::sleep(Duration::from_millis(250));
    let _ = line.discard_pending_raw_bytes()?;
    let request = format!("{REQUEST_PREFIX}{expected_build}");
    line.send_raw_stream_frame(request.as_bytes(), Duration::from_secs(2))?;
    let mut response = [0_u8; 1024];
    let response = line.receive_raw_stream_frame(&mut response, Duration::from_secs(3))?;
    let record: serde_json::Value = serde_json::from_slice(response)?;
    println!("{}", serde_json::to_string(&record)?);
    if record["schema"] != RESPONSE_SCHEMA
        || record["build_id"] != expected_build
        || record["success"] != true
        || record["window_ms"] != 1_000
        || record["translator_final_level"] != "low"
        || record["uart_tx_bytes"] != 0
        || record["motion_authority_granted"] != false
    {
        return Err(format!("bounded Create RX listen failed: {record}").into());
    }
    Ok(())
}
