//! HIL-only collection of terminal and two Chromium control inputs.

use std::process::{Child, Command};
use std::time::Duration;

use conduit_core::Plan;
use conduit_std_host::pico_control_source::PicoControlSource;
use conduit_std_host::usb_cdc::NativePathCdcLineReader;

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
            r1_control_session::deliver_input(io, source, input, &mut |sequence| {
                let line = clue
                    .read_line(Duration::from_secs(3))
                    .map_err(|error| format!("missing live Plan A physical LED Sign: {error}"))?;
                super::r1_signal_transcript::verify_receipt(
                    &line, plan, sequence, identity, runtime,
                )
            })
            .map_err(|error| error.to_string())
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
