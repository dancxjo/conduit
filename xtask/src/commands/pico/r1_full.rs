//! One-boot physical R1 orchestration across both recovery branches.

use conduit_std_host::usb_cdc::{NativePathCdcCarrier, NativePathCdcLineReader};

use super::firmware::FirmwareIdentity;
use super::transcript::RuntimeTranscriptIdentity;
use super::PicoResult;

pub fn run(
    carrier: &mut NativePathCdcCarrier,
    clue: &mut NativePathCdcLineReader,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
    interactive: bool,
) -> PicoResult<()> {
    let lifecycle = super::prove_websocket::verify_new_plan_recovery(
        carrier,
        clue,
        identity,
        runtime,
        interactive,
    )?;
    let body_id = lifecycle.body.body_id.clone();
    println!("==> Restore real Wi-Fi/network availability for Plan C, then press Enter");
    let mut confirmation = String::new();
    std::io::stdin().read_line(&mut confirmation)?;
    let final_lifecycle = super::prove_websocket::verify_plan_c_continuation(
        carrier,
        clue,
        identity,
        runtime,
        interactive,
        Some(lifecycle),
    )?;
    if final_lifecycle.body.body_id != body_id {
        return Err("combined R1 proof changed Body identity between branches".into());
    }
    println!(
        "{}",
        serde_json::json!({
            "schema": "conduit.r1/complete-hil@1",
            "proof_class": "physical-cross-host",
            "body_id": body_id.as_str(),
            "pico_host_id": conduit_net::R1_PICO_HOST_ID,
            "pico_boot_id": runtime.boot_id.as_str(),
            "new_plan_recovery_completed": true,
            "same_plan_continuation_completed": true,
            "body_lulled_after_each_branch": true,
            "combined_physical_acceptance": true,
        })
    );
    println!("==> Combined physical R1 new-Plan and same-Plan recovery completed");
    Ok(())
}
