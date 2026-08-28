//! Physical R1 proof for one USB-authorized Pico W infrastructure attachment.

use std::time::{Duration, Instant};

#[cfg(unix)]
use conduit_std_host::pico_wifi_bootstrap::PicoWifiBootstrapSource;
#[cfg(unix)]
use conduit_std_host::usb_cdc::{NativePathCdcLine, NativePathCdcLineReader};
use conduit_wire::{SessionMessage, SessionTerminalDisposition};

use super::doctor::repo_root;
use super::firmware::{read_identity_manifest, FirmwareIdentity};
use super::serial::resolve_dual_ports;
use super::transcript::{self, RuntimeTranscriptIdentity};
use super::wifi_secrets::SecretEnvValue;
use super::{PicoArgs, PicoResult};
use crate::cli::GlobalOpts;

#[derive(Clone)]
pub enum WifiProofMode {
    Bootstrap,
    WebSocketRoute,
    R1NewPlanRecovery {
        interactive: bool,
    },
    R1PlanCContinuation {
        interactive: bool,
    },
    R1Full {
        interactive: bool,
        membership_receipt: Option<std::path::PathBuf>,
    },
}

impl WifiProofMode {
    fn firmware_mode(&self) -> &'static str {
        match self {
            Self::R1NewPlanRecovery { .. }
            | Self::R1PlanCContinuation { .. }
            | Self::R1Full { .. } => "r1-control",
            Self::Bootstrap | Self::WebSocketRoute => "wifi-bootstrap",
        }
    }
}

pub fn run_prove_pico_wifi_bootstrap(
    link_port_opt: Option<&str>,
    sign_port_opt: Option<&str>,
    ssid_env: Option<&str>,
    credential_env: Option<&str>,
    mode: WifiProofMode,
    pico_args: &PicoArgs,
    opts: &GlobalOpts,
) -> PicoResult<()> {
    if opts.dry_run || pico_args.dry_run {
        let proof = match mode {
            WifiProofMode::Bootstrap => "pico-wifi-bootstrap",
            WifiProofMode::WebSocketRoute => "pico-websocket-route",
            WifiProofMode::R1NewPlanRecovery { .. } => "r1-new-plan-recovery-hil",
            WifiProofMode::R1PlanCContinuation { .. } => "r1-plan-c-continuation-hil",
            WifiProofMode::R1Full { .. } => "r1-hil",
        };
        println!("==> prove {proof} (dry-run)");
        println!("  firmware mode: {}", mode.firmware_mode());
        println!(
            "  link port: {}",
            link_port_opt.unwrap_or("<auto-discover CDC 0>")
        );
        println!(
            "  sign port: {}",
            sign_port_opt.unwrap_or("<auto-discover CDC 1>")
        );
        println!(
            "  SSID source: {}",
            ssid_env.unwrap_or("<required --ssid-env variable name>")
        );
        println!(
            "  credential source: {}",
            credential_env.unwrap_or("<required --credential-env variable name>")
        );
        println!("  secret values are never printed or serialized into the Plan");
        if matches!(mode, WifiProofMode::R1Full { .. }) {
            println!("  stage 1: live three-peer Plan A to automatic USB Plan B recovery and Lull");
            println!("  operator action: restore real Wi-Fi/network availability");
            println!("  stage 2: same Body later Wake runs Plan C continuation and Lull");
        }
        return Ok(());
    }

    let ssid_name = ssid_env.ok_or("--ssid-env is required for physical Wi-Fi proof")?;
    let credential_name =
        credential_env.ok_or("--credential-env is required for physical Wi-Fi proof")?;
    let ssid = SecretEnvValue::read(ssid_name)?;
    let credential = SecretEnvValue::read(credential_name)?;

    let (link_port_path, sign_port_path) = match resolve_dual_ports(link_port_opt, sign_port_opt) {
        Ok(ports) => ports,
        Err(_) => {
            println!("==> Active Pico W CDC ports not found. Flashing Wi-Fi bootstrap image...");
            super::flash::run_flash(pico_args)?;
            println!("==> Waiting for USB CDC ports to enumerate...");
            std::thread::sleep(Duration::from_secs(3));
            resolve_dual_ports(link_port_opt, sign_port_opt)?
        }
    };
    println!(
        "==> prove pico-wifi-bootstrap: link port {}, sign port {}",
        link_port_path.display(),
        sign_port_path.display()
    );

    let identity = read_identity_manifest(&repo_root())?;
    let expected_firmware_mode = mode.firmware_mode();
    if identity.firmware_mode != expected_firmware_mode {
        return Err(format!(
            "physical Wi-Fi proof requires a {expected_firmware_mode} image, but the current artifact is {}; rebuild and flash with matching Pico mode flags",
            identity.firmware_mode,
        )
        .into());
    }
    let generated = &identity.generated_image;
    let expected_schema = if expected_firmware_mode == "r1-control" {
        identity.verified_r1_control_images()?;
        "conduit-pico-w-signal/identity@2"
    } else {
        if identity.r1_control_images.is_some() {
            return Err("ordinary Wi-Fi bootstrap identity contains control images".into());
        }
        "conduit-pico-w-signal/identity@1"
    };
    if identity.schema != expected_schema
        || generated.schema != "conduit.pico-network.generated-image@1"
        || generated.firmware_mode != identity.firmware_mode
        || generated.firmware_build_id != identity.firmware_build_id
        || generated.nodes != 2
        || generated.cords != 2
        || generated.host_operations != 2
        || generated.cord_value_slots != 2
        || generated.cord_value_bytes
            != conduit_net::MAXIMUM_JOIN_INPUT_BYTES + conduit_net::MAXIMUM_JOIN_OUTPUT_BYTES
    {
        return Err("Wi-Fi bootstrap generated-image identity or fixed bounds are invalid".into());
    }

    #[cfg(not(unix))]
    {
        let _ = (ssid, credential, identity);
        return Err("physical Pico Wi-Fi proof currently requires a Unix CDC host".into());
    }

    #[cfg(unix)]
    {
        run_unix(
            &link_port_path,
            &sign_port_path,
            ssid,
            credential,
            &identity,
            mode,
        )?;
    }

    Ok(())
}

#[cfg(unix)]
fn run_unix(
    link_port: &std::path::Path,
    sign_port: &std::path::Path,
    ssid: SecretEnvValue,
    credential: SecretEnvValue,
    identity: &FirmwareIdentity,
    mode: WifiProofMode,
) -> PicoResult<()> {
    let mut sign = NativePathCdcLineReader::open(sign_port)
        .map_err(|error| format!("failed to open CDC1 sign port: {error}"))?;
    let mut line = NativePathCdcLine::open(link_port, 1024)
        .map_err(|error| format!("failed to open CDC0 link port: {error}"))?;
    std::thread::sleep(Duration::from_millis(250));

    line.send_raw_stream_frame(b"CONDUIT_RAW_CDC0_PROBE", Duration::from_secs(2))?;
    let mut raw = [0_u8; 2048];
    if line.receive_raw_stream_frame(&mut raw, Duration::from_secs(3))? != b"CONDUIT_RAW_CDC0_REPLY"
    {
        return Err("raw CDC0 probe reply payload mismatch".into());
    }
    let boot_line = sign
        .read_line(Duration::from_secs(3))
        .map_err(|error| format!("timed out reading Pico boot Sign: {error}"))?;
    let runtime = transcript::verify_boot(&boot_line, identity)?;

    match wait_for_network_session_readiness(&mut line)? {
        NetworkSessionReadiness::Ready => {}
        NetworkSessionReadiness::Failed => {
            return Err(read_recovery_sign(&mut line, &mut sign, identity, &runtime));
        }
    }

    let mut source = PicoWifiBootstrapSource::prepare(ssid.bytes(), credential.bytes())
        .map_err(|error| format!("failed to prepare bounded credential source: {error}"))?;
    drop(ssid);
    drop(credential);
    verify_source_identity(&source, identity)?;
    source.observe_sink_boot(conduit_core::BootId::from(runtime.boot_id.as_str()))?;
    let binding = source.binding().clone();
    if binding.sink_active_play_id.as_str() != runtime.active_play_id {
        return Err("runtime Pico Play identity disagrees with the exact rebound Plan".into());
    }

    if let Err(handshake_error) = handshake(&mut source, &mut line, &binding) {
        return Err(session_phase_failure_with_sign(
            &mut sign,
            "USB bootstrap handshake",
            handshake_error,
            identity,
            &runtime,
        ));
    }
    let (sequence, payload, payload_len) = source
        .next_offer()?
        .ok_or("credential source produced no runtime Info")?;
    if sequence != 0 {
        return Err("credential source emitted a non-zero first sequence".into());
    }
    let offered = binding.frame(SessionMessage::Offered {
        sequence,
        payload: &payload[..payload_len],
    });
    source.admit_outbound(offered)?;
    line.send_frame(&offered, Duration::from_secs(2))?;
    receive_exact(
        &mut line,
        &mut source,
        Duration::from_secs(3),
        |message| matches!(message, SessionMessage::Accepted { sequence: found } if found == sequence),
    )?;
    source.accepted(sequence)?;

    let attachment_line = sign.read_line(Duration::from_secs(65)).map_err(|error| {
        format!("timed out waiting for network attachment/failure Sign: {error}")
    })?;
    verify_attachment_sign(&attachment_line, identity, &runtime)?;
    receive_exact(
        &mut line,
        &mut source,
        Duration::from_secs(3),
        |message| matches!(message, SessionMessage::Delivered { sequence: found } if found == sequence),
    )?;
    source.delivered(sequence)?;
    if source.next_offer()?.is_some() || source.finish_kernel()? != 1 {
        return Err("credential source did not reach its exact single-value terminal".into());
    }

    let input_closed = binding.frame(SessionMessage::InputClosed { final_sequence: 1 });
    source.admit_outbound(input_closed)?;
    line.send_frame(&input_closed, Duration::from_secs(2))?;
    let terminal = binding.frame(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Completed,
        final_sequence: 1,
    });
    source.admit_outbound(terminal)?;
    line.send_frame(&terminal, Duration::from_secs(2))?;
    receive_exact(&mut line, &mut source, Duration::from_secs(3), |message| {
        matches!(
            message,
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence: 1,
            }
        )
    })?;
    if !source.is_terminal() {
        return Err("USB bootstrap session did not reach reciprocal terminal agreement".into());
    }

    match mode {
        WifiProofMode::Bootstrap => {
            super::usb_continuity::verify(&mut line, identity)?;
            println!(
                "==> Physical network attachment Sign and post-attachment USB continuity verified"
            );
        }
        WifiProofMode::WebSocketRoute => {
            super::prove_websocket::verify(&mut line, &mut sign, identity, &runtime)?;
        }
        WifiProofMode::R1NewPlanRecovery { interactive } => {
            let _ = super::prove_websocket::verify_new_plan_recovery(
                &mut line,
                &mut sign,
                identity,
                &runtime,
                interactive,
            )?;
        }
        WifiProofMode::R1PlanCContinuation { interactive } => {
            let _ = super::prove_websocket::verify_plan_c_continuation(
                &mut line,
                &mut sign,
                identity,
                &runtime,
                interactive,
                None,
            )?;
        }
        WifiProofMode::R1Full {
            interactive,
            membership_receipt,
        } => {
            super::r1_full::run(
                &mut line,
                &mut sign,
                identity,
                &runtime,
                interactive,
                membership_receipt.as_deref(),
            )?;
        }
    }
    Ok(())
}

#[cfg(unix)]
enum NetworkSessionReadiness {
    Ready,
    Failed,
}

#[cfg(unix)]
fn wait_for_network_session_readiness(
    line: &mut NativePathCdcLine,
) -> PicoResult<NetworkSessionReadiness> {
    line.send_raw_stream_frame(
        conduit_r1_network_conformance::R1_USB_NETWORK_SESSION_QUERY,
        Duration::from_secs(2),
    )?;
    let mut raw = [0_u8; 1024];
    let reply = line.receive_raw_stream_frame(&mut raw, Duration::from_secs(30))?;
    if reply == conduit_r1_network_conformance::R1_USB_NETWORK_SESSION_FAILED {
        return Ok(NetworkSessionReadiness::Failed);
    }
    if reply != conduit_r1_network_conformance::R1_USB_NETWORK_SESSION_READY {
        return Err("Pico returned an unexpected network Session readiness payload".into());
    }
    Ok(NetworkSessionReadiness::Ready)
}

#[cfg(unix)]
fn read_recovery_sign(
    line: &mut NativePathCdcLine,
    sign: &mut NativePathCdcLineReader,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> Box<dyn std::error::Error> {
    std::thread::scope(|scope| -> Box<dyn std::error::Error> {
        let disposition = scope.spawn(|| -> Result<Vec<u8>, String> {
            line.send_raw_stream_frame(
                conduit_r1_network_conformance::R1_USB_NETWORK_FAILURE_SIGN_READY,
                Duration::from_secs(2),
            )
            .map_err(|error| error.to_string())?;
            let mut raw = [0_u8; 1024];
            Ok(line
                .receive_raw_stream_frame(&mut raw, Duration::from_secs(3))
                .map_err(|error| error.to_string())?
                .to_vec())
        });
        let line = match sign.read_line(Duration::from_secs(3)) {
            Ok(line) => line,
            Err(error) => return format!("no bounded Pico recovery Sign arrived: {error}").into(),
        };
        let status = match disposition.join() {
            Ok(Ok(status)) => status,
            Ok(Err(error)) => return error.into(),
            Err(_) => return "Pico recovery disposition reader panicked".into(),
        };
        if status == conduit_r1_network_conformance::R1_USB_NETWORK_FAILURE_SIGN_FORMAT_FAILED {
            return "Pico recovery Sign exceeded its admitted format bound".into();
        }
        if status == conduit_r1_network_conformance::R1_USB_NETWORK_FAILURE_SIGN_DISCONNECTED {
            return "Pico recovery Sign face disconnected during delivery".into();
        }
        if status.as_slice() != conduit_r1_network_conformance::R1_USB_NETWORK_FAILURE_SIGN_WRITTEN
        {
            return "Pico returned an unexpected recovery Sign disposition".into();
        }
        match verify_attachment_sign(&line, identity, runtime) {
            Ok(()) => "Pico emitted an unexpected attachment success Sign".into(),
            Err(error) => format!("Pico failure Sign: {error}").into(),
        }
    })
}

#[cfg(unix)]
fn session_phase_failure_with_sign(
    sign: &mut NativePathCdcLineReader,
    phase: &str,
    phase_error: Box<dyn std::error::Error>,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> Box<dyn std::error::Error> {
    match sign.read_line(Duration::from_secs(3)) {
        Ok(line) => match verify_attachment_sign(&line, identity, runtime) {
            Ok(()) => format!(
                "{phase} failed ({phase_error}); Pico emitted an unexpected attachment success Sign"
            )
            .into(),
            Err(sign_error) => {
                format!("{phase} failed ({phase_error}); Pico failure Sign: {sign_error}").into()
            }
        },
        Err(sign_error) => format!(
            "{phase} failed ({phase_error}); no bounded Pico failure Sign arrived: {sign_error}"
        )
        .into(),
    }
}

#[cfg(unix)]
fn verify_source_identity(
    source: &PicoWifiBootstrapSource,
    identity: &FirmwareIdentity,
) -> PicoResult<()> {
    let binding = source.binding();
    let generated = &identity.generated_image;
    if binding.plan_id.as_str() != generated.plan_id
        || binding.sink_fragment_id.as_str() != generated.fragment_id
        || binding.sink.host_id.as_str() != generated.host_id
        || binding.sink.boot_id.as_str() != generated.boot_id
        || binding.attachment.base
            != conduit_core::BaseImplementationId::from("conduit.base/usb-cdc-acm@1")
    {
        return Err(
            "running firmware is not the exact Pico fragment/USB Line of the bootstrap Plan".into(),
        );
    }
    Ok(())
}

#[cfg(unix)]
fn handshake(
    source: &mut PicoWifiBootstrapSource,
    line: &mut NativePathCdcLine,
    binding: &conduit_wire::SessionBinding,
) -> PicoResult<()> {
    let hello = binding.hello_frame();
    source.admit_outbound(hello)?;
    line.send_frame(&hello, Duration::from_secs(2))?;
    receive_exact(line, source, Duration::from_secs(5), |message| {
        matches!(message, SessionMessage::Hello(_))
    })?;
    let ready = binding.frame(SessionMessage::Ready);
    source.admit_outbound(ready)?;
    line.send_frame(&ready, Duration::from_secs(2))?;
    receive_exact(line, source, Duration::from_secs(5), |message| {
        matches!(message, SessionMessage::Ready)
    })?;
    if !source.is_active() {
        return Err("USB bootstrap session did not become active".into());
    }
    Ok(())
}

#[cfg(unix)]
fn receive_exact(
    line: &mut NativePathCdcLine,
    source: &mut PicoWifiBootstrapSource,
    timeout: Duration,
    expected: impl Fn(SessionMessage<'_>) -> bool,
) -> PicoResult<()> {
    let deadline = Instant::now() + timeout;
    let mut frame = [0_u8; 2048];
    while Instant::now() < deadline {
        match line.receive_frame(&mut frame, Duration::from_millis(100)) {
            Ok(received) if expected(received.message) => {
                source.admit_inbound(received)?;
                return Ok(());
            }
            Ok(_) => return Err("Pico returned an unexpected session frame".into()),
            Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
            Err(error) => {
                return Err(format!("failed receiving Pico session frame: {error:?}").into())
            }
        }
    }
    Err("timed out waiting for the exact Pico session frame".into())
}

#[cfg(unix)]
fn verify_attachment_sign(
    line: &str,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let record: serde_json::Value = serde_json::from_str(line)
        .map_err(|error| format!("malformed bounded network Sign JSON: {error}"))?;
    let schema = record["schema"].as_str();
    if schema == Some("conduit.network/recovery-failure-sign@1") {
        for (field, expected) in [
            ("firmware_build_id", identity.firmware_build_id.as_str()),
            ("runtime_boot_id", runtime.boot_id.as_str()),
            ("runtime_active_play_id", runtime.active_play_id.as_str()),
            (
                "sign_id",
                identity.generated_image.terminal_sign_id.as_str(),
            ),
        ] {
            if record[field].as_str() != Some(expected) {
                return Err(format!("network recovery Sign field `{field}` mismatched").into());
            }
        }
        let code = record["error_code"]
            .as_str()
            .unwrap_or("missing-error-code");
        return Err(format!("Pico network recovery reported bounded Sign `{code}`").into());
    }
    if !matches!(
        schema,
        Some("conduit.network/attachment-sign@1" | "conduit.network/join-failure-sign@1")
    ) {
        return Err("unexpected network Sign schema".into());
    }
    let generated = &identity.generated_image;
    for (field, expected) in [
        ("firmware_build_id", identity.firmware_build_id.as_str()),
        ("source_document_id", generated.source_document_id.as_str()),
        ("checked_form_id", generated.checked_form_id.as_str()),
        ("expanded_form_id", generated.expanded_form_id.as_str()),
        ("plan_id", generated.plan_id.as_str()),
        ("fragment_id", generated.fragment_id.as_str()),
        ("host_id", generated.host_id.as_str()),
        ("boot_id", runtime.boot_id.as_str()),
        ("active_play_id", runtime.active_play_id.as_str()),
        (
            "interface_pool_id",
            conduit_r1_network_conformance::R1_WIFI_STATION_POOL_ID,
        ),
        ("sign_id", generated.terminal_sign_id.as_str()),
    ] {
        if record[field].as_str() != Some(expected) {
            return Err(format!("network attachment Sign field `{field}` mismatched").into());
        }
    }
    if schema == Some("conduit.network/join-failure-sign@1") {
        let code = record["error_code"]
            .as_str()
            .unwrap_or("missing-error-code");
        return Err(format!("Pico network join failed with bounded Sign `{code}`").into());
    }
    if record["generation"].as_u64() != Some(1)
        || record["attachment_id"].as_str().is_none_or(str::is_empty)
    {
        return Err("network attachment Sign identity is incomplete".into());
    }
    Ok(())
}

#[cfg(all(test, unix))]
#[path = "prove_wifi_tests.rs"]
mod tests;
