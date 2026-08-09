//! HIL-only collection of terminal and two Chromium control inputs.

use std::process::{Child, Command};
use std::time::Duration;

use conduit_core::Plan;
use conduit_std_host::pico_control_source::PicoControlSource;
use conduit_std_host::r1_control::{R1ControlPeer, R1MergedInput};
use conduit_std_host::usb_cdc::NativePathCdcLineReader;
use serde::Serialize;

use super::firmware::FirmwareIdentity;
use super::r1_control_session;
use super::r1_signal::R1SessionIo;
use super::transcript::RuntimeTranscriptIdentity;
use super::PicoResult;

pub fn deliver_plan_a_inputs(
    io: &mut impl R1SessionIo,
    source: &mut PicoControlSource,
    clue: &mut NativePathCdcLineReader,
    plan: &Plan,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let mut browser: Option<BrowserHarness> = None;
    conduit_std_host::r1_control_input::run_live_three_peer_events(
        "127.0.0.1:0",
        |address| {
            browser = Some(BrowserHarness::spawn(address)?);
            println!("==> Two Chromium peers are starting; enter 'down' then 'up' on separate lines for the terminal peer");
            Ok(())
        },
        |input| {
            let merged = r1_control_session::deliver_input(io, source, input, &mut |sequence| {
                let line = clue
                    .read_line(Duration::from_secs(3))
                    .map_err(|error| format!("missing live Plan A physical LED Sign: {error}"))?;
                super::r1_signal_transcript::verify_receipt(
                    &line, plan, sequence, input.level, identity, runtime,
                )
            })
            .map_err(|error| error.to_string())?;
            emit_physical_input_sign(plan, &merged).map_err(|error| error.to_string())?;
            Ok(merged)
        },
    )
    .map_err(|error| format!("live R1 control input failed: {error}"))?;
    browser
        .take()
        .ok_or("live R1 browser harness was not started")?
        .wait()
        .map_err(|error| format!("live R1 browser harness: {error}"))?;
    Ok(())
}

pub fn emit_physical_input_sign(plan: &Plan, merged: &R1MergedInput) -> PicoResult<()> {
    println!(
        "{}",
        serde_json::to_string(&PhysicalInputSign::new(plan, merged))?
    );
    Ok(())
}

#[derive(Serialize)]
struct PhysicalInputSign<'a> {
    schema: &'static str,
    proof_class: &'static str,
    plan_id: &'a str,
    peer: &'static str,
    peer_sequence: u64,
    input: &'static str,
    requested_level: bool,
    merged_sequence: u64,
    physical_led_result: &'static str,
    evidence: &'static str,
}

impl<'a> PhysicalInputSign<'a> {
    fn new(plan: &'a Plan, merged: &R1MergedInput) -> Self {
        Self {
            schema: "conduit.r1/physical-input-sign@1",
            proof_class: "physical-cross-host",
            plan_id: plan.plan_id.as_str(),
            peer: match merged.input.peer {
                R1ControlPeer::Terminal => "terminal",
                R1ControlPeer::BrowserA => "browser-a",
                R1ControlPeer::BrowserB => "browser-b",
            },
            peer_sequence: merged.input.peer_sequence,
            input: if merged.input.level {
                "keydown"
            } else {
                "keyup"
            },
            requested_level: merged.input.level,
            merged_sequence: merged.signal.sequence,
            physical_led_result: if merged.signal.level { "on" } else { "off" },
            evidence: "verified-pico-receipt",
        }
    }
}

struct BrowserHarness(Option<Child>);

impl BrowserHarness {
    fn spawn(address: std::net::SocketAddr) -> Result<Self, String> {
        let child = Command::new("npx")
            .args([
                "playwright",
                "test",
                "--config",
                "hosts/browser/r1-physical-input.playwright.config.mjs",
            ])
            .env("CONDUIT_R1_INPUT_LINE", format!("ws://{address}"))
            .spawn()
            .map_err(|error| format!("start live R1 Chromium harness: {error}"))?;
        Ok(Self(Some(child)))
    }

    fn wait(mut self) -> Result<(), String> {
        let status = self
            .0
            .take()
            .ok_or_else(|| "live R1 Chromium harness process missing".to_string())?
            .wait()
            .map_err(|error| format!("wait for Chromium harness: {error}"))?;
        if !status.success() {
            return Err(format!("Chromium harness exited with {status}"));
        }
        Ok(())
    }
}

impl Drop for BrowserHarness {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use conduit_signal::Signal;
    use conduit_std_host::r1_control::R1InputEvent;

    use super::*;

    #[test]
    fn physical_sign_correlates_exact_peer_sequence_plan_and_led_result() {
        let plan = conduit_system_continuity::exact_r1_control_plan(
            conduit_core::BootId::from(conduit_net::R1_PICO_BOOT_ID),
            conduit_system_continuity::R1SignalRouteSet::UsbOnly,
        )
        .unwrap()
        .plan;
        let merged = R1MergedInput {
            input: R1InputEvent {
                peer: R1ControlPeer::BrowserB,
                peer_sequence: 1,
                level: false,
            },
            signal: Signal {
                sequence: 5,
                level: false,
            },
        };
        let sign = serde_json::to_value(PhysicalInputSign::new(&plan, &merged)).unwrap();
        assert_eq!(sign["plan_id"], plan.plan_id.as_str());
        assert_eq!(sign["peer"], "browser-b");
        assert_eq!(sign["peer_sequence"], 1);
        assert_eq!(sign["merged_sequence"], 5);
        assert_eq!(sign["input"], "keyup");
        assert_eq!(sign["physical_led_result"], "off");
        assert_eq!(sign["evidence"], "verified-pico-receipt");
    }
}
