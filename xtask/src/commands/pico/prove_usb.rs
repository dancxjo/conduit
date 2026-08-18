//! Typed std > Pico W USB-CDC session proof & interactive console runner.

use std::time::{Duration, Instant};

use conduit_signal::decode_signal_bytes;
#[cfg(unix)]
use conduit_std_host::pico_usb_source::PicoUsbSource;
#[cfg(unix)]
use conduit_std_host::usb_cdc::{NativePathCdcLine, NativePathCdcLineReader, OperatorTerminal};
use conduit_wire::{SessionBinding, SessionMessage};

use super::doctor::repo_root;
use super::firmware::read_identity_manifest;
use super::serial::resolve_dual_ports;
use super::transcript::{self, RuntimeTranscriptIdentity};
use super::{PicoArgs, PicoResult};
use crate::cli::GlobalOpts;

pub fn run_prove_std_pico_usb(
    link_port_opt: Option<&str>,
    sign_port_opt: Option<&str>,
    interactive: bool,
    induce_sink_failure: bool,
    pico_args: &PicoArgs,
    opts: &GlobalOpts,
) -> PicoResult<()> {
    if opts.dry_run || pico_args.dry_run {
        println!("==> prove std-pico-usb (dry-run)");
        println!("  firmware build: cargo check --manifest-path firmware/conduit-pico-w-signal/Cargo.toml --target thumbv6m-none-eabi");
        println!("  flash candidate: RPI-RP2 mount copy");
        println!(
            "  link port: {}",
            link_port_opt.unwrap_or("<auto-discover CDC 0>")
        );
        println!(
            "  sign port: {}",
            sign_port_opt.unwrap_or("<auto-discover CDC 1>")
        );
        println!("  interactive console: {}", interactive);
        println!("  induced sink failure: {}", induce_sink_failure);
        return Ok(());
    }

    // 1. Resolve dual CDC ports if board is already running, or build+flash if not
    let (link_port_path, sign_port_path) = match resolve_dual_ports(link_port_opt, sign_port_opt) {
        Ok(ports) => {
            println!("==> Detected active Pico W dual CDC ports");
            ports
        }
        Err(_) => {
            println!("==> Active Pico W CDC ports not found. Flashing firmware candidate...");
            super::flash::run_flash(pico_args)?;
            println!("==> Waiting for USB CDC serial ports to enumerate...");
            std::thread::sleep(Duration::from_secs(3));
            resolve_dual_ports(link_port_opt, sign_port_opt)?
        }
    };

    println!(
        "==> prove std-pico-usb: link port {}, sign port {}",
        link_port_path.display(),
        sign_port_path.display()
    );

    // Read expected compiled firmware identity manifest
    let root = repo_root();
    let identity = read_identity_manifest(&root)?;
    if identity.firmware_mode != "usb-remote" {
        return Err(format!(
            "std-to-Pico USB proof requires a usb-remote image, but the current artifact is {}; rebuild with `cargo xtask pico build --usb-remote` and flash it with `cargo xtask pico flash --usb-remote`",
            identity.firmware_mode
        )
        .into());
    }
    println!(
        "==> Loaded build identity: build_id={}, plan_id={}",
        identity.firmware_build_id, identity.generated_image.plan_id
    );

    #[cfg(unix)]
    {
        let mut attempts = 0;
        let (mut sign_reader, mut line, runtime) = loop {
            attempts += 1;
            match (|| -> Result<_, String> {
                // 1. Open CDC 1 and require the kernel to report DTR high.
                let sign_reader = NativePathCdcLineReader::open(&sign_port_path)
                    .map_err(|e| format!("Failed to open CDC1 sign port: {e}"))?;
                println!("==> CDC1 opened; DTR verified high");

                // 2. Open CDC 0 link port and verify DTR there independently.
                let mut line = NativePathCdcLine::open(&link_port_path, 1024)
                    .map_err(|e| format!("Failed to open CDC0 link port: {e}"))?;
                println!("==> CDC0 opened; DTR verified high");

                // 3. Settle 250 ms to race host open against Pico firmware USB loop
                std::thread::sleep(Duration::from_millis(250));

                // 4. Raw CDC0 Physical Checkpoint
                line.send_raw_stream_frame(b"CONDUIT_RAW_CDC0_PROBE", Duration::from_secs(2))
                    .map_err(|e| format!("Failed to send raw CDC0 probe: {e}"))?;
                println!("  [Source] Sent raw CDC0 stream frame probe");

                let mut frame_buf = [0u8; 2048];
                let probe_reply = line
                    .receive_raw_stream_frame(&mut frame_buf, Duration::from_secs(3))
                    .map_err(|e| format!("Timed out waiting for CDC0 probe reply: {e}"))?;
                if probe_reply == b"CONDUIT_RAW_CDC0_REPLY" {
                    println!("==> CDC0 raw Pico > host reply observed");
                } else {
                    return Err("raw CDC0 probe reply payload mismatch".into());
                }

                // 5. Read Pico W boot identity from CDC 1
                let mut reader = sign_reader;
                let boot_line = reader
                    .read_line(Duration::from_secs(3))
                    .map_err(|e| format!("Timed out reading boot identity from CDC 1: {e}"))?;
                println!("==> CDC1 boot identity received and validated");

                let runtime = transcript::verify_boot(&boot_line, &identity)
                    .map_err(|error| error.to_string())?;

                let gpio_ready = reader
                    .read_line(Duration::from_secs(10))
                    .map_err(|e| format!("Timed out waiting for CYW43 GPIO readiness: {e}"))?;
                if gpio_ready != "CONDUIT_CYW43_GPIO_READY" {
                    return Err(format!("unexpected CDC1 startup record: {gpio_ready}"));
                }
                println!("==> Pico CYW43 GPIO service ready");

                Ok((reader, line, runtime))
            })() {
                Ok(res) => break res,
                Err(err) => {
                    if attempts >= 3 {
                        return Err(format!(
                            "Connection attempt failed after {attempts} retries: {err}"
                        )
                        .into());
                    }
                    println!("==> Connection attempt {attempts} failed ({err}); retrying fresh connection in 250 ms...");
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
        };

        println!(
            "==> Observed Pico W runtime link/boot identity: boot_id={}, play_id={}",
            runtime.boot_id, runtime.active_play_id
        );

        let mut source = PicoUsbSource::prepare()
            .map_err(|error| format!("failed to prepare exact std kernel source: {error}"))?;
        let binding = source.binding().clone();
        if binding.plan_id.as_str() != identity.generated_image.plan_id
            || binding.sink_fragment_id.as_str() != identity.generated_image.fragment_id
            || binding.sink.host_id.as_str() != identity.generated_image.host_id
            || binding.sink.boot_id.as_str() != identity.generated_image.boot_id
        {
            return Err(
                "running firmware image is not the sink fragment of the exact std kernel plan"
                    .into(),
            );
        }
        source.observe_sink_boot(conduit_core::BootId::from(runtime.boot_id.as_str()))?;
        let binding = source.binding().clone();
        if binding.sink_active_play_id.as_str() != runtime.active_play_id {
            return Err(
                "observed Pico boot produced inconsistent session/runtime play identity".into(),
            );
        }
        println!(
            "==> Exact generated plan installed: plan={}, source_fragment={}, sink_fragment={}, link={}",
            binding.plan_id.as_str(),
            binding.source_fragment_id.as_str(),
            binding.sink_fragment_id.as_str(),
            binding.attachment.link_binding_id.as_str(),
        );

        // Handshake Step 1 (Source): Outbound Hello
        let hello = binding.hello_frame();
        source
            .admit_outbound(hello)
            .map_err(|e| format!("Source failed to admit outbound Hello: {e:?}"))?;
        line.send_frame(&hello, Duration::from_secs(2))?;
        println!("==> std  > pico Hello");

        // Handshake Step 2 (Source): Inbound Hello
        let mut frame_buf = [0u8; 2048];
        let start = Instant::now();
        let mut hello_received = false;
        while start.elapsed() < Duration::from_secs(5) {
            match line.receive_frame(&mut frame_buf, Duration::from_millis(100)) {
                Ok(res) => {
                    if matches!(res.message, SessionMessage::Hello(_)) {
                        source.admit_inbound(res).map_err(|e| {
                            format!("Source failed to admit inbound Hello from Sink: {e:?}")
                        })?;
                        println!("==> pico > std  Hello");
                        hello_received = true;
                        break;
                    }
                }
                Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
                Err(err) => {
                    println!("  [Source SessionMachine] receive_frame error: {:?}", err);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        if !hello_received {
            return Err("timed out waiting for SessionMessage::Hello from Pico W".into());
        }

        // Handshake Step 3 (Source): Outbound Ready
        let ready_outbound = binding.frame(SessionMessage::Ready);
        source
            .admit_outbound(ready_outbound)
            .map_err(|e| format!("Source failed to admit outbound Ready: {e:?}"))?;
        line.send_frame(&ready_outbound, Duration::from_secs(2))?;
        println!("==> std  > pico Ready");

        // Handshake Step 4 (Source): Inbound Ready
        let start = Instant::now();
        let mut ready_received = false;
        while start.elapsed() < Duration::from_secs(5) {
            match line.receive_frame(&mut frame_buf, Duration::from_millis(100)) {
                Ok(res) => {
                    if matches!(res.message, SessionMessage::Ready) {
                        source.admit_inbound(res).map_err(|e| {
                            format!("Source failed to admit inbound Ready from Sink: {e:?}")
                        })?;
                        println!("==> pico > std  Ready");
                        ready_received = true;
                        break;
                    }
                }
                Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
                Err(err) => {
                    println!("  [Source SessionMachine] receive_frame error: {:?}", err);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        if !ready_received {
            return Err("timed out waiting for SessionMessage::Ready from Pico W".into());
        }

        if !source.is_active() {
            return Err("Source SessionMachine is not active after 4-message handshake".into());
        }

        if interactive && induce_sink_failure {
            return Err("interactive and induced sink-failure modes are mutually exclusive".into());
        }

        if interactive {
            println!("\n===============================================================");
            println!(" Conduit USB-CDC Pico W Interactive Control (#350)");
            println!(" Link Port:     {}", link_port_path.display());
            println!(" Sign Port: {}", sign_port_path.display());
            println!("===============================================================");
            println!(
                " [Press ANY KEY]  > Instant Button Pulse (Key Down > LED ON > Key Up > LED OFF)"
            );
            println!(" [Press 'q' / ESC] > Exit interactive session");
            println!("===============================================================\n");

            let mut term = OperatorTerminal::open().map_err(|e| {
                format!("Failed to initialize interactive operator terminal: {}", e)
            })?;
            while let Some((sequence, payload)) = source.next_offer()? {
                let key = match term.read_key(Duration::from_millis(100)) {
                    Ok(Some(k)) => k,
                    Ok(None) => continue,
                    Err(_) => break,
                };

                if key == b'q' || key == b'Q' || key == 3 || key == 27 {
                    return Err("interactive exit requested before the exact kernel plan reached terminal; cancellation is not yet an accepted #465 path".into());
                }
                let signal = decode_signal_bytes(&payload).map_err(|error| error.to_string())?;
                println!(
                    "  [KEY 0x{:02x}] releasing planned kernel Signal seq {} (level: {})",
                    key, sequence, signal.level
                );
                send_and_verify_item(
                    &mut source,
                    ItemProofContext {
                        line: &mut line,
                        sign_reader: &mut sign_reader,
                        binding: &binding,
                        identity: &identity,
                        runtime: &runtime,
                    },
                    sequence,
                    &payload,
                    signal.level,
                )?;
            }
            let final_sequence = source.finish_kernel()?;
            super::session_completion::complete(
                &mut source,
                &mut line,
                &mut sign_reader,
                &binding,
                final_sequence,
                &identity,
                &runtime,
            )?;
            println!("==> Pico W USB-CDC interactive session completed.");
            return Ok(());
        }

        println!("==> Executing the std source fragment through conduit-kernel...");
        let mut observed = 0_u64;
        while let Some((sequence, payload)) = source.next_offer()? {
            if sequence != observed {
                return Err(format!("std kernel emitted out-of-order sequence {sequence}").into());
            }
            let signal = decode_signal_bytes(&payload).map_err(|error| error.to_string())?;
            if signal.sequence != sequence {
                return Err(
                    "std kernel Signal payload sequence disagrees with remote offer".into(),
                );
            }
            if induce_sink_failure {
                if sequence != 0 {
                    return Err("sink-failure probe did not terminate on the first value".into());
                }
                let mut invalid_payload = payload;
                // Keep the payload width valid but make its semantic sequence
                // disagree with the exact kernel offer identity.
                invalid_payload[0] = 1;
                super::session_failure::complete_induced_sink_failure(
                    &mut source,
                    &mut line,
                    &binding,
                    sequence,
                    &invalid_payload,
                )?;
                let terminal = sign_reader
                    .read_line(Duration::from_secs(2))
                    .map_err(|error| format!("missing Pico terminal failure sign: {error}"))?;
                transcript::verify_terminal_failure(&terminal, &identity, &runtime)?;
                println!(
                    "==> Induced Pico sink failure reached reciprocal Failed and terminal agreement"
                );
                return Ok(());
            }
            send_and_verify_item(
                &mut source,
                ItemProofContext {
                    line: &mut line,
                    sign_reader: &mut sign_reader,
                    binding: &binding,
                    identity: &identity,
                    runtime: &runtime,
                },
                sequence,
                &payload,
                signal.level,
            )?;
            observed += 1;
        }
        let final_sequence = source.finish_kernel()?;
        if observed != final_sequence {
            return Err(format!(
                "std kernel terminal sequence {final_sequence} disagrees with {observed} delivered values"
            )
                .into());
        }
        super::session_completion::complete(
            &mut source,
            &mut line,
            &mut sign_reader,
            &binding,
            final_sequence,
            &identity,
            &runtime,
        )?;
        println!("==> Pico W USB-CDC proof completed successfully.");
    }

    Ok(())
}

#[cfg(unix)]
struct ItemProofContext<'a> {
    line: &'a mut NativePathCdcLine,
    sign_reader: &'a mut NativePathCdcLineReader,
    binding: &'a SessionBinding,
    identity: &'a super::firmware::FirmwareIdentity,
    runtime: &'a RuntimeTranscriptIdentity,
}

#[cfg(unix)]
fn send_and_verify_item(
    source: &mut PicoUsbSource,
    context: ItemProofContext<'_>,
    sequence: u64,
    payload: &[u8; conduit_signal::SIGNAL_ENCODED_LEN as usize],
    level: bool,
) -> PicoResult<()> {
    let offer = context
        .binding
        .frame(SessionMessage::Offered { sequence, payload });

    let mut frame_buf = [0u8; 2048];
    let mut pressure_retries = 0_u8;
    loop {
        // The source kernel retains ownership until Accepted. Pressure permits
        // only this exact offer to be retried, once, by the proof policy.
        source
            .admit_outbound(offer)
            .map_err(|e| format!("Source failed to admit outbound Offered: {e:?}"))?;
        context.line.send_frame(&offer, Duration::from_secs(2))?;

        let start = Instant::now();
        let mut retry = false;
        let mut accepted = false;
        while start.elapsed() < Duration::from_secs(2) {
            match context
                .line
                .receive_frame(&mut frame_buf, Duration::from_millis(100))
            {
                Ok(res) => {
                    if matches!(res.message, SessionMessage::Accepted { sequence: s } if s == sequence)
                    {
                        source.admit_inbound(res).map_err(|e| {
                            format!("Source failed to admit inbound Accepted: {e:?}")
                        })?;
                        source.accepted(sequence)?;
                        accepted = true;
                        break;
                    }
                    if matches!(res.message, SessionMessage::Pressure { sequence: s } if s == sequence)
                    {
                        source.admit_inbound(res).map_err(|e| {
                            format!("Source failed to admit inbound Pressure: {e:?}")
                        })?;
                        source.pressure(sequence)?;
                        pressure_retries += 1;
                        if pressure_retries > 1 {
                            return Err(
                                "Pico repeated pressure beyond the proof's admitted retry policy"
                                    .into(),
                            );
                        }
                        retry = true;
                        break;
                    }
                }
                Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
                Err(err) => {
                    return Err(format!("receive_frame error waiting for Accepted: {err:?}").into());
                }
            }
        }
        if accepted {
            break;
        }
        if !retry {
            return Err(
                format!("timed out waiting for SessionMessage::Accepted {sequence}").into(),
            );
        }
    }

    // 3. Read receipt line from CDC 1
    let line = context
        .sign_reader
        .read_line(Duration::from_secs(2))
        .map_err(|e| {
            format!("timed out reading receipt for sequence {sequence} from CDC 1: {e}")
        })?;
    transcript::verify_receipt(
        &line,
        sequence as usize,
        level,
        context.identity,
        context.runtime,
    )?;
    println!("  [RECEIPT ] <- CDC 1: sequence={sequence}, level={level}, identity=verified");

    // 4. Wait for Delivered(sequence) over CDC 0
    let start_del = Instant::now();
    let mut delivered = false;
    while start_del.elapsed() < Duration::from_secs(2) {
        match context
            .line
            .receive_frame(&mut frame_buf, Duration::from_millis(100))
        {
            Ok(res) => {
                if matches!(res.message, SessionMessage::Delivered { sequence: s } if s == sequence)
                {
                    source
                        .admit_inbound(res)
                        .map_err(|e| format!("Source failed to admit inbound Delivered: {e:?}"))?;
                    source.delivered(sequence)?;
                    delivered = true;
                    break;
                }
            }
            Err(conduit_std_host::usb_cdc::NativeUsbCdcError::WouldBlock) => {}
            Err(err) => {
                return Err(format!("receive_frame error waiting for Delivered: {err:?}").into());
            }
        }
    }
    if !delivered {
        return Err(format!("timed out waiting for SessionMessage::Delivered {sequence}").into());
    }

    Ok(())
}
