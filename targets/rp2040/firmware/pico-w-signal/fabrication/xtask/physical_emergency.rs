use std::time::{Duration, Instant};

use conduit_body::{BodyId, EmergencyMachineAction, EmergencyPolicy, EmergencyStepOutcome};
use conduit_core::{BootId, HostId};
use conduit_pete::{PetePhysicalEmergencyAdapter, PetePhysicalEmergencyReceipt};
use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::capstone_serial::resolve_port as resolve_capstone_port;
use super::doctor::repo_root;
use super::firmware::identity_manifest_path;
use super::{PicoArgs, PicoResult};

const QUERY_PREFIX: &str = "CONDUIT_PHYSICAL_EMERGENCY_QUERY@1:";
const ATTENDED_DEADLINE: Duration = Duration::from_secs(30);

pub fn run(args: &PicoArgs, body_id: &str, host_id: &str, boot_id: &str) -> PicoResult<()> {
    if !args.pete_capstone {
        return Err("pico physical-emergency requires --pete-capstone".into());
    }
    let identity: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        identity_manifest_path(&repo_root()),
    )?)?;
    let expected_build = identity["firmware_build_id"]
        .as_str()
        .ok_or("capstone image identity missing firmware_build_id; rebuild the image")?;
    let body_id: BodyId = serde_json::from_value(serde_json::Value::String(body_id.into()))?;
    let policy = EmergencyPolicy {
        allow_keyboard_rescue: false,
        allow_physical: true,
        allow_acoustic: false,
        allow_remote: false,
        attempt_graceful_lull: false,
        revoke_local_execution: true,
        isolate_carriers: false,
        terminal_action: EmergencyMachineAction::None,
    };
    let mut adapter = PetePhysicalEmergencyAdapter::admit(
        expected_build,
        body_id,
        HostId::from(host_id),
        BootId::from(boot_id),
        policy,
    )
    .map_err(|error| format!("physical emergency admission refused: {error:?}"))?;
    if args.dry_run {
        println!(
            "==> pico physical-emergency (dry-run): arm exact build/body/host/boot binding, wait at most 30 seconds for GPIO22 active-low, verify local Create authority revocation"
        );
        return Ok(());
    }

    let port = resolve_capstone_port(args.port.as_deref())?;
    let mut line = NativePathCdcLine::open(&port, 1024)?;
    std::thread::sleep(Duration::from_millis(250));
    let _ = line.discard_pending_raw_bytes()?;
    line.send_raw_stream_frame(adapter.arm_command().as_bytes(), Duration::from_secs(2))?;
    let initial = receive_receipt(&mut line)?;
    if initial.build_id != expected_build
        || initial.binding_sha256 != adapter.binding_sha256()
        || initial.triggered
    {
        return Err("firmware did not arm the exact untriggered emergency binding".into());
    }
    println!("==> pico physical-emergency: armed; press Pete's dedicated GPIO22 emergency switch");

    let query = format!("{QUERY_PREFIX}{expected_build}");
    let deadline = Instant::now() + ATTENDED_DEADLINE;
    while Instant::now() < deadline {
        line.send_raw_stream_frame(query.as_bytes(), Duration::from_secs(2))?;
        let receipt = receive_receipt(&mut line)?;
        if receipt.triggered {
            let outcome = adapter
                .inspect(&receipt)
                .map_err(|error| format!("physical emergency receipt refused: {error:?}"))?;
            if outcome.local_execution_revocation != EmergencyStepOutcome::Completed
                || outcome.machine_action_requested
            {
                return Err(
                    "physical switch exceeded or failed its admitted local reduction".into(),
                );
            }
            println!("{}", serde_json::to_string(&receipt)?);
            println!(
                "==> pico physical-emergency: one-shot local Create authority reduction proved"
            );
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("physical emergency switch was not observed within 30 seconds".into())
}

fn receive_receipt(line: &mut NativePathCdcLine) -> PicoResult<PetePhysicalEmergencyReceipt> {
    let mut response = [0_u8; 1024];
    let response = line.receive_raw_stream_frame(&mut response, Duration::from_secs(2))?;
    Ok(serde_json::from_slice(response)?)
}
